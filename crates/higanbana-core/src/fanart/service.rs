//! Capa de um anime pelo fanart.tv: mapa (lista Fribb) → imagens → arquivo local reduzido.
//! Erro de rede nunca vai para as caches; "sem pôster" fica um dia, o pôster escolhido uma semana.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::{
    FanartError, FanartResult,
    client::FanartClient,
    encode::shrink,
    mapping::{AnimeMap, Target},
    pick::{pick_movie, pick_tv},
    store::CoverStore,
};
use crate::anilist::{MediaType, cache::DiskCache, types::MediaSummary};

const FRIBB_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json";
const MAP_TTL: i64 = 7 * 24 * 3600;
const HITS_TTL: i64 = 7 * 24 * 3600;
const MISS_TTL: i64 = 24 * 3600;
/// Depois de uma falha ao baixar a lista, só tenta de novo passado este tempo.
const MAP_RETRY: Duration = Duration::from_secs(10 * 60);
/// A lista tem vários MB.
const MAP_TIMEOUT: Duration = Duration::from_secs(60);

/// Mapa em disco: quando foi baixado e o conteúdo.
#[derive(Serialize, Deserialize)]
struct MapFile {
    at: i64,
    map: AnimeMap,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Chave das caches por alvo.
fn target_key(t: Target) -> String {
    match t {
        Target::Season { tvdb, season, series_poster } => format!("tv:{tvdb}:{season}:{series_poster}"),
        Target::Movie { tmdb } => format!("movie:{tmdb}"),
    }
}

pub struct Fanart {
    client: FanartClient,
    /// Só para baixar a lista Fribb (timeout maior que o do cliente).
    http: reqwest::Client,
    map: RwLock<Option<Arc<AnimeMap>>>,
    map_path: PathBuf,
    /// Quando a última tentativa de baixar a lista falhou.
    map_failed: Mutex<Option<Instant>>,
    /// Alvo → URL do pôster escolhido.
    hits: DiskCache<String>,
    /// Alvos sem pôster no fanart.tv.
    misses: DiskCache<bool>,
    store: CoverStore,
}

impl Fanart {
    pub fn new(cache_dir: &Path) -> Self {
        Self::with_parts(FanartClient::new(), None, cache_dir)
    }

    /// `map` já carregado dispensa o download da lista (testes).
    pub(crate) fn with_parts(client: FanartClient, map: Option<AnimeMap>, cache_dir: &Path) -> Self {
        Self {
            client,
            http: reqwest::Client::builder()
                .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
                .timeout(MAP_TIMEOUT)
                .build()
                .expect("falha ao criar cliente HTTP"),
            map: RwLock::new(map.map(Arc::new)),
            map_path: cache_dir.join("fanart-map.json"),
            map_failed: Mutex::new(None),
            hits: DiskCache::open(cache_dir.join("fanart-hits.json"), HITS_TTL),
            misses: DiskCache::open(cache_dir.join("fanart-miss.json"), MISS_TTL),
            store: CoverStore::open(cache_dir.join("covers")),
        }
    }

    /// Caminho local do pôster, sem rede. `None` = ainda não tem (ou usar AniList).
    pub fn cover_cached(&self, m: &MediaSummary) -> Option<PathBuf> {
        if m.media_type == Some(MediaType::Manga) {
            return None;
        }
        let map = self.map.try_read().ok()?.clone()?;
        let key = target_key(map.target(m.id, m.id_mal)?);
        self.store.path_for(&self.hits.get(&key)?)
    }

    /// Resolve mapa → fanart → arquivo. `Ok(None)` = usar AniList.
    pub async fn cover(&self, m: &MediaSummary) -> FanartResult<Option<PathBuf>> {
        if m.media_type == Some(MediaType::Manga) || !self.client.configured() {
            return Ok(None);
        }
        let map = self.load_map().await?;
        let Some(target) = map.target(m.id, m.id_mal) else { return Ok(None) };
        let result = self.resolve(target).await;
        self.hits.flush();
        self.misses.flush();
        result
    }

    /// Apaga um arquivo de capa inválido.
    pub fn discard(&self, path: &Path) {
        self.store.discard(path);
    }

    async fn resolve(&self, target: Target) -> FanartResult<Option<PathBuf>> {
        let key = target_key(target);
        if self.misses.get(&key).is_some() {
            return Ok(None);
        }
        let url = match self.hits.get(&key) {
            Some(url) => {
                if let Some(path) = self.store.path_for(&url) {
                    return Ok(Some(path));
                }
                url
            }
            None => {
                let picked = match target {
                    Target::Season { tvdb, season, series_poster } => {
                        pick_tv(&self.client.tv(tvdb).await?, season, series_poster)
                    }
                    Target::Movie { tmdb } => pick_movie(&self.client.movie(tmdb).await?),
                };
                let Some(url) = picked else {
                    self.misses.insert(key, true);
                    return Ok(None);
                };
                url
            }
        };
        let bytes = self.client.download(&url).await?;
        let Ok((small, ext)) = shrink(&bytes) else {
            // Imagem ilegível: não adianta baixar de novo por um dia.
            self.misses.insert(key, true);
            return Ok(None);
        };
        let path = self.store.save(&url, &small, ext).map_err(|e| FanartError::Image(e.to_string()))?;
        self.hits.insert(key, url);
        Ok(Some(path))
    }

    /// Mapa carregado; baixa/renova a lista quando preciso. Chamadas concorrentes
    /// esperam a primeira em vez de baixar em paralelo.
    async fn load_map(&self) -> FanartResult<Arc<AnimeMap>> {
        if let Some(map) = self.map.read().await.clone() {
            return Ok(map);
        }
        let mut slot = self.map.write().await;
        if let Some(map) = slot.clone() {
            return Ok(map);
        }
        let old = std::fs::read(&self.map_path).ok().and_then(|b| serde_json::from_slice::<MapFile>(&b).ok());
        let fresh = old.as_ref().is_some_and(|f| now() - f.at < MAP_TTL);
        if fresh {
            let map = Arc::new(old.expect("conferido acima").map);
            *slot = Some(map.clone());
            return Ok(map);
        }
        let waiting = self.map_failed.lock().unwrap().is_some_and(|t| t.elapsed() < MAP_RETRY);
        let fetched = if waiting {
            Err(FanartError::Http("lista de mapeamento indisponível, nova tentativa em breve".into()))
        } else {
            self.fetch_map().await
        };
        match fetched {
            Ok(map) => {
                *self.map_failed.lock().unwrap() = None;
                self.save_map(&map);
                let map = Arc::new(map);
                *slot = Some(map.clone());
                Ok(map)
            }
            Err(e) => {
                if !waiting {
                    *self.map_failed.lock().unwrap() = Some(Instant::now());
                }
                // Lista vencida ainda serve melhor que nada.
                let map = Arc::new(old.ok_or(e)?.map);
                *slot = Some(map.clone());
                Ok(map)
            }
        }
    }

    async fn fetch_map(&self) -> FanartResult<AnimeMap> {
        let resp = self.http.get(FRIBB_URL).send().await.map_err(|e| FanartError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(FanartError::Http(format!("HTTP {}", resp.status())));
        }
        let bytes = resp.bytes().await.map_err(|e| FanartError::Http(e.to_string()))?;
        tokio::task::spawn_blocking(move || AnimeMap::from_fribb(&bytes))
            .await
            .map_err(|e| FanartError::Decode(e.to_string()))?
    }

    /// Melhor esforço: sem gravar, só baixa de novo na próxima vez.
    fn save_map(&self, map: &AnimeMap) {
        let Ok(bytes) = serde_json::to_vec(&serde_json::json!({ "at": now(), "map": map })) else { return };
        let tmp = self.map_path.with_extension("json.tmp");
        if let Some(dir) = self.map_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &self.map_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    /// Pasta única por teste, apagada ao sair de escopo.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            Self(std::env::temp_dir().join(format!("higanbana-fanart-{}-{n}-{nanos}", std::process::id())))
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn media(id: i64, kind: &str) -> MediaSummary {
        serde_json::from_str(&format!(r#"{{"id":{id},"type":"{kind}"}}"#)).unwrap()
    }

    /// Mapa com AniList 5 → temporada 1 da série TVDB 5.
    fn map() -> AnimeMap {
        let json = r#"[{"type":"TV","anilist_id":5,"tvdb_id":5,"season":{"tvdb":1}}]"#;
        AnimeMap::from_fribb(json.as_bytes()).unwrap()
    }

    #[tokio::test]
    async fn manga_returns_none_without_network() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(Some("k".into())), Some(map()), &t.0);
        assert!(matches!(f.cover(&media(5, "MANGA")).await, Ok(None)));
    }

    #[tokio::test]
    async fn unconfigured_returns_none() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(None), None, &t.0);
        assert!(matches!(f.cover(&media(5, "ANIME")).await, Ok(None)));
        assert!(f.map.read().await.is_none(), "mapa não pode ser baixado sem chave");
    }

    #[tokio::test]
    async fn unmapped_returns_none() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(Some("k".into())), Some(map()), &t.0);
        assert!(matches!(f.cover(&media(999, "ANIME")).await, Ok(None)));
    }

    #[tokio::test]
    async fn cached_miss_returns_none_without_network() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(Some("k".into())), Some(map()), &t.0);
        f.misses.insert("tv:5:1:true".into(), true);
        assert!(matches!(f.cover(&media(5, "ANIME")).await, Ok(None)));
    }

    #[test]
    fn cached_hit_with_file_is_returned_sync() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(None), Some(map()), &t.0);
        let url = "https://assets.fanart.tv/a.jpg";
        f.hits.insert("tv:5:1:true".into(), url.into());
        let path = f.store.save(url, b"x", "webp").unwrap();
        assert_eq!(f.cover_cached(&media(5, "ANIME")), Some(path));
    }

    #[test]
    fn cached_hit_without_file_is_none() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(None), Some(map()), &t.0);
        f.hits.insert("tv:5:1:true".into(), "https://assets.fanart.tv/a.jpg".into());
        assert_eq!(f.cover_cached(&media(5, "ANIME")), None);
    }

    #[test]
    fn cached_without_map_is_none() {
        let t = Tmp::new();
        let f = Fanart::with_parts(FanartClient::with_key(None), None, &t.0);
        assert_eq!(f.cover_cached(&media(5, "ANIME")), None);
    }
}
