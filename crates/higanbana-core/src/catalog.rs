//! Confere se um anime do AniList está no acervo. A interface pede só quando o
//! card aparece na tela. Economia de API:
//! - lista a pasta da letra (`Animes/Letra X`) uma vez e compara nomes localmente;
//! - nomes parecidos são confirmados no AniList (lote + cache em disco);
//! - no máximo 2 conferências em paralelo;
//! - cada título é conferido uma vez; resultado guardado por 12 h.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    anilist::{AniList, LookupRequest, MediaType, types::MediaSummary},
    anime_index::AnimeIndex,
    cloud::Cloud,
    library::{folder_title, norm},
};

const TTL_SECS: u64 = 12 * 3600;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    at: u64,
    /// id → caminho (ou `None`: não está no acervo).
    paths: HashMap<i64, Option<String>>,
}

pub struct Catalog {
    file: PathBuf,
    stored: Mutex<Stored>,
    limit: Semaphore,
}

/// Remove sufixos de temporada/subtítulo: "Foo Season 2", "Foo 2nd Season", "Foo: Bar".
pub fn base_title(s: &str) -> String {
    let s = s.split([':', '：']).next().unwrap_or(s);
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)\s+(\d+(st|nd|rd|th)\s+season|season\s+\d+|part\s+\d+|cour\s+\d+)\b.*$").unwrap()
    });
    RE.replace(s, "").trim().to_owned()
}

fn titles_of(m: &MediaSummary) -> Vec<String> {
    let t = &m.title;
    [t.romaji.clone(), t.english.clone(), t.user_preferred.clone()]
        .into_iter()
        .flatten()
        .chain(m.synonyms.iter().cloned())
        .filter(|s| s.chars().any(|c| c.is_ascii_alphabetic()))
        .collect()
}

/// Pasta de letra onde o título estaria: `Letra B` ou `0-9`.
pub fn letter_folder(title: &str) -> Option<String> {
    let c = norm(title).chars().next()?;
    Some(if c.is_ascii_digit() { "0-9".into() } else { format!("Letra {}", c.to_ascii_uppercase()) })
}

impl Catalog {
    pub fn open(file: PathBuf) -> Self {
        let stored: Stored = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|s: &Stored| now().saturating_sub(s.at) < TTL_SECS)
            .unwrap_or_else(|| Stored { at: now(), paths: HashMap::new() });
        Self { file, stored: Mutex::new(stored), limit: Semaphore::new(2) }
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

    async fn resolve(&self, index: &AnimeIndex, cloud: &Cloud, anilist: &AniList, m: &MediaSummary) -> Result<Option<String>, String> {
        let titles = titles_of(m);
        let exact: Vec<String> = titles.iter().map(|t| norm(t)).collect();
        let bases: Vec<String> = titles.iter().map(|t| norm(&base_title(t))).filter(|b| b.len() >= 3).collect();
        let mut letters: Vec<String> = titles.iter().take(2).filter_map(|t| letter_folder(t)).collect();
        letters.dedup();

        let mut near = Vec::new();
        for letter in &letters {
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
        // Nome parecido (ex.: sequência numa pasta com o nome base): confirma pelo AniList.
        for (name, path) in &near {
            let found = anilist
                .lookup(&[LookupRequest { name: name.clone(), media_type: MediaType::Anime }])
                .await
                .ok()
                .and_then(|v| v.into_iter().next().flatten());
            if found.and_then(|f| f.get("id").and_then(|v| v.as_i64())) == Some(m.id) {
                return Ok(Some(path.clone()));
            }
        }
        // A temporada nova costuma ficar dentro da pasta da obra base.
        Ok(near.into_iter().next().map(|(_, p)| p))
    }

    /// Confere um título no acervo. Falha de rede não é guardada.
    pub async fn check(&self, index: &AnimeIndex, cloud: &Cloud, anilist: &AniList, m: &MediaSummary) -> Option<String> {
        if let Some(known) = self.known(m.id) {
            return known;
        }
        let _permit = self.limit.acquire().await.ok()?;
        if let Some(known) = self.known(m.id) {
            return known;
        }
        let path = self.resolve(index, cloud, anilist, m).await.ok()?;
        self.stored.lock().unwrap().paths.insert(m.id, path.clone());
        self.save();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
