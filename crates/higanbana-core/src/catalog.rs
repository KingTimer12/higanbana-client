//! Confere se uma obra do AniList (anime ou mangá) está no acervo. A interface
//! pede só quando o card aparece na tela. Economia de API:
//! - lista a pasta da letra da seção uma vez e compara nomes localmente;
//! - nomes parecidos são confirmados no AniList (lote + cache em disco);
//! - dentro da pasta base, as subpastas seguem as regras de [`crate::catalog_rules`];
//! - listagens de pasta base ficam em disco por 12 h;
//! - no máximo 2 conferências em paralelo;
//! - cada título é conferido uma vez; resultado guardado por 12 h.

use std::{
    collections::HashMap,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    anilist::{AniList, Lookup, LookupRequest, MediaType, cache::DiskCache, types::MediaSummary},
    anime_index::AnimeIndex,
    catalog_rules::{Candidate, RULES, titles_of},
    cloud::Cloud,
    http::HttpResult,
    library::{folder_title, natural_cmp, norm},
    nuvem::Listing,
};

const TTL_SECS: u64 = 12 * 3600;
/// Muda quando a conferência muda de regra: descarta o que foi gravado antes
/// (a versão 1 marcava sequências sem pasta como "no acervo").
pub const CATALOG_VERSION: u32 = 2;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    v: u32,
    at: u64,
    /// id → caminho (ou `None`: não está no acervo).
    paths: HashMap<i64, Option<String>>,
}

/// Quantas listagens da nuvem uma tela ainda pode pedir.
pub struct ListBudget {
    left: AtomicU32,
}

impl ListBudget {
    pub fn new(n: u32) -> Arc<Self> {
        Arc::new(Self { left: AtomicU32::new(n) })
    }

    /// Gasta uma listagem; `false` quando acabou.
    pub fn take(&self) -> bool {
        self.left.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok()
    }

    pub fn left(&self) -> u32 {
        self.left.load(Ordering::SeqCst)
    }
}

const NO_BUDGET: &str = "orçamento de listagens esgotado";

pub struct Catalog {
    file: PathBuf,
    stored: Mutex<Stored>,
    limit: Semaphore,
    /// Subpastas de pastas base já listadas.
    folders: DiskCache<Vec<String>>,
}

/// Remove sufixos de temporada/subtítulo: "Foo Season 2", "Foo 2nd Season", "Foo: Bar".
pub fn base_title(s: &str) -> String {
    let s = s.split([':', '：']).next().unwrap_or(s);
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)\s+(\d+(st|nd|rd|th)\s+season|season\s+\d+|part\s+\d+|cour\s+\d+)\b.*$").unwrap()
    });
    RE.replace(s, "").trim().to_owned()
}

/// Pasta de letra onde o título estaria: `Letra B` ou `0-9`.
pub fn letter_folder(title: &str) -> Option<String> {
    let c = norm(title).chars().next()?;
    Some(if c.is_ascii_digit() { "0-9".into() } else { format!("Letra {}", c.to_ascii_uppercase()) })
}

/// Qual nome resolveu para `id`. Achou → `Ok(Some(posição))`; todos respondidos
/// sem achar → `Ok(None)`; algum ainda sem resposta (`Pending`) → `Err`.
pub(crate) fn pick_confirmed(results: Vec<Lookup>, id: i64) -> Result<Option<usize>, String> {
    let mut pending = false;
    for (i, l) in results.into_iter().enumerate() {
        match l {
            Lookup::Found(f) if f.get("id").and_then(|v| v.as_i64()) == Some(id) => return Ok(Some(i)),
            Lookup::Pending => pending = true,
            _ => {}
        }
    }
    if pending { Err("AniList sem resposta".into()) } else { Ok(None) }
}

impl Catalog {
    pub fn open(file: PathBuf, folders: PathBuf) -> Self {
        let stored: Stored = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|s: &Stored| s.v == CATALOG_VERSION && now().saturating_sub(s.at) < TTL_SECS)
            .unwrap_or_else(|| Stored { v: CATALOG_VERSION, at: now(), paths: HashMap::new() });
        Self { file, stored: Mutex::new(stored), limit: Semaphore::new(2), folders: DiskCache::open(folders, TTL_SECS as i64) }
    }

    /// Já conferido: `Some(Some(caminho))`, `Some(None)` (não está), `None` (não conferido).
    pub fn known(&self, id: i64) -> Option<Option<String>> {
        self.stored.lock().unwrap().paths.get(&id).cloned()
    }

    fn save(&self) {
        let bytes = serde_json::to_vec(&*self.stored.lock().unwrap());
        if let Ok(b) = bytes {
            let _ = std::fs::write(&self.file, b);
        }
    }

    /// Subpastas de uma pasta base: do cache ou de uma listagem (gasta orçamento).
    async fn subdirs(&self, cloud: &Cloud, path: &str, budget: Option<&ListBudget>) -> Result<Vec<String>, String> {
        if let Some(hit) = self.folders.get(path) {
            return Ok(hit);
        }
        if budget.is_some_and(|b| !b.take()) {
            return Err(NO_BUDGET.into());
        }
        // Em caixa: corta a cadeia de tipos (senão a checagem de `Send` estoura no desktop).
        let listing: Pin<Box<dyn Future<Output = HttpResult<Listing>> + Send + '_>> = Box::pin(cloud.list_background(path));
        let l = listing.await.map_err(|e| e.to_string())?;
        let mut names: Vec<String> = l.files.into_iter().filter(|f| f.is_directory).map(|f| f.name).collect();
        names.sort_by(|a, b| natural_cmp(a, b));
        self.folders.insert(path.to_owned(), names.clone());
        self.folders.flush();
        Ok(names)
    }

    /// Posição do primeiro nome que resolve no AniList para `id` (lote, cache).
    /// Sem resposta do AniList → `Err` (não vira "não está no acervo").
    async fn confirms(anilist: &AniList, names: &[String], media: MediaType, id: i64) -> Result<Option<usize>, String> {
        let reqs: Vec<LookupRequest> = names.iter().map(|n| LookupRequest { name: n.clone(), media_type: media }).collect();
        let found = anilist.lookup(&reqs).await.map_err(|e| e.to_string())?;
        pick_confirmed(found, id)
    }

    async fn resolve(
        &self,
        index: &AnimeIndex,
        cloud: &Cloud,
        anilist: &AniList,
        m: &MediaSummary,
        budget: Option<&ListBudget>,
    ) -> Result<Option<String>, String> {
        let media = m.media_type.unwrap_or(MediaType::Anime);
        let titles = titles_of(m);
        let exact: Vec<String> = titles.iter().map(|t| norm(t)).collect();
        let bases: Vec<String> = titles.iter().map(|t| norm(&base_title(t))).filter(|b| b.len() >= 3).collect();
        let mut letters: Vec<String> = titles.iter().take(2).filter_map(|t| letter_folder(t)).collect();
        letters.dedup();

        let mut near = Vec::new();
        for letter in &letters {
            if index.loaded(letter).is_none() && budget.is_some_and(|b| !b.take()) {
                return Err(NO_BUDGET.into());
            }
            for entry in index.letter(cloud, letter).await?.iter() {
                let (name, path) = (&entry.name, entry.path.clone());
                let n = norm(&folder_title(name));
                if exact.contains(&n) {
                    return Ok(Some(path));
                }
                if bases.contains(&n) {
                    near.push((name.clone(), path));
                }
            }
        }
        // Nome parecido: a própria pasta base pode ser a obra (confirma pelo AniList).
        let names: Vec<String> = near.iter().map(|(n, _)| n.clone()).collect();
        if !names.is_empty()
            && let Some(i) = Self::confirms(anilist, &names, media, m.id).await?
        {
            return Ok(Some(near[i].1.clone()));
        }
        // Senão, procura a obra dentro da pasta base, pelas regras de formato.
        for (_, base) in &near {
            let dirs = self.subdirs(cloud, base, budget).await?;
            for rule in RULES {
                let mut confirm = Vec::new();
                for c in rule(base, &dirs, m) {
                    match c {
                        Candidate::Path(p) => return Ok(Some(p)),
                        Candidate::Confirm { name, path } => confirm.push((name, path)),
                    }
                }
                if confirm.is_empty() {
                    continue;
                }
                let names: Vec<String> = confirm.iter().map(|(n, _)| n.clone()).collect();
                if let Some(i) = Self::confirms(anilist, &names, media, m.id).await? {
                    return Ok(Some(confirm[i].1.clone()));
                }
            }
        }
        Ok(None)
    }

    /// Confere um título no acervo. Falha de rede ou orçamento esgotado não é
    /// guardado. Obras só da MAL (id negativo) não são conferidas.
    pub async fn check(
        &self,
        index: &AnimeIndex,
        cloud: &Cloud,
        anilist: &AniList,
        m: &MediaSummary,
        budget: Option<&ListBudget>,
    ) -> Option<String> {
        if m.is_mal_only() {
            return None;
        }
        if let Some(known) = self.known(m.id) {
            return known;
        }
        let _permit = self.limit.acquire().await.ok()?;
        if let Some(known) = self.known(m.id) {
            return known;
        }
        let path = self.resolve(index, cloud, anilist, m, budget).await.ok()?;
        self.stored.lock().unwrap().paths.insert(m.id, path.clone());
        self.save();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("catalog-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn old_cache_without_version_is_dropped() {
        let dir = tmp("old");
        let file = dir.join("catalog.json");
        std::fs::write(&file, format!(r#"{{"at": {}, "paths": {{"5": "Animes/Letra A/X"}}}}"#, now())).unwrap();
        assert_eq!(Catalog::open(file, dir.join("folders.json")).known(5), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn current_version_cache_is_kept() {
        let dir = tmp("cur");
        let file = dir.join("catalog.json");
        std::fs::write(&file, format!(r#"{{"v": {CATALOG_VERSION}, "at": {}, "paths": {{"5": "Animes/Letra A/X"}}}}"#, now())).unwrap();
        assert_eq!(Catalog::open(file, dir.join("folders.json")).known(5), Some(Some("Animes/Letra A/X".into())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn found(id: i64) -> Lookup {
        Lookup::Found(serde_json::json!({ "id": id }))
    }

    #[test]
    fn confirm_pending_is_not_a_negative() {
        assert!(pick_confirmed(vec![found(9), Lookup::Pending], 1).is_err());
    }

    #[test]
    fn confirm_all_answered_without_match_is_negative() {
        assert_eq!(pick_confirmed(vec![found(9), Lookup::NotFound], 1), Ok(None));
    }

    #[test]
    fn confirm_match_wins_even_with_pending() {
        assert_eq!(pick_confirmed(vec![Lookup::Pending, found(1)], 1), Ok(Some(1)));
    }

    #[test]
    fn budget_takes_until_zero() {
        let b = ListBudget::new(2);
        assert!(b.take());
        assert!(b.take());
        assert!(!b.take());
        assert_eq!(b.left(), 0);
    }

    #[test]
    fn base_titles_and_letters() {
        assert_eq!(base_title("Shingeki no Kyojin Season 3"), "Shingeki no Kyojin");
        assert_eq!(base_title("Re:Zero 2nd Season"), "Re");
        assert_eq!(base_title("Dr. Stone: New World"), "Dr. Stone");
        assert_eq!(letter_folder("86 Eighty-Six").as_deref(), Some("0-9"));
        assert_eq!(letter_folder("Ōkami").as_deref(), Some("Letra O"));
        assert_eq!(letter_folder("!!!"), None);
    }
}
