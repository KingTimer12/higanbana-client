//! Índice das pastas de obra de `Animes/<letra>`. Uma listagem por vez, com
//! 400 ms entre listagens de rede e recuo exponencial em HTTP 429 (a nuvem
//! limita ~50 listagens seguidas). Guardado em disco por 12 h. A varredura
//! pode ser pausada (vídeo tocando); a pausa vale só entre letras.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::{
    cloud::Cloud,
    http::HttpError,
    library::{join_path, natural_cmp},
};

const ROOT: &str = "Animes";
const TTL_SECS: u64 = 12 * 3600;
const SPACING: Duration = Duration::from_millis(400);
const MAX_ATTEMPTS: u32 = 6;

pub const LETTERS: [&str; 27] = [
    "0-9", "Letra A", "Letra B", "Letra C", "Letra D", "Letra E", "Letra F", "Letra G", "Letra H", "Letra I", "Letra J", "Letra K",
    "Letra L", "Letra M", "Letra N", "Letra O", "Letra P", "Letra Q", "Letra R", "Letra S", "Letra T", "Letra U", "Letra V", "Letra W",
    "Letra X", "Letra Y", "Letra Z",
];

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Espera antes de repetir uma listagem recusada com 429: 5 s, 10 s, 20 s, 40 s, 60 s, 60 s…
pub fn backoff(attempt: u32) -> Duration {
    Duration::from_secs((5u64 << attempt.min(4)).min(60))
}

/// Quanto falta para respeitar o intervalo desde a última listagem.
fn remaining(last: Option<Instant>, now: Instant, gap: Duration) -> Duration {
    last.map_or(Duration::ZERO, |l| gap.saturating_sub(now.saturating_duration_since(l)))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexEntry {
    pub letter: String,
    pub name: String,
    pub path: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    at: u64,
    letters: HashMap<String, Vec<IndexEntry>>,
}

#[derive(Default)]
struct State {
    at: u64,
    letters: HashMap<String, Arc<Vec<IndexEntry>>>,
    collections: Vec<String>,
}

pub struct AnimeIndex {
    file: PathBuf,
    state: Mutex<State>,
    /// Uma listagem de rede por vez; guarda o instante da última (espaçamento global).
    net: tokio::sync::Mutex<Option<Instant>>,
    paused: AtomicBool,
    resume: Notify,
}

impl AnimeIndex {
    pub fn open(file: PathBuf) -> Self {
        let stored: Stored = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|s: &Stored| now().saturating_sub(s.at) < TTL_SECS)
            .unwrap_or_else(|| Stored { at: now(), letters: HashMap::new() });
        let state = State {
            at: stored.at,
            letters: stored.letters.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
            collections: Vec::new(),
        };
        Self { file, state: Mutex::new(state), net: Default::default(), paused: AtomicBool::new(false), resume: Notify::new() }
    }

    /// Letra já indexada (memória ou disco), sem rede.
    pub fn loaded(&self, letter: &str) -> Option<Arc<Vec<IndexEntry>>> {
        let st = self.state.lock().unwrap();
        if now().saturating_sub(st.at) >= TTL_SECS {
            return None;
        }
        st.letters.get(letter).cloned()
    }

    /// Letras carregadas e total de letras.
    pub fn progress(&self) -> (usize, usize) {
        let st = self.state.lock().unwrap();
        (LETTERS.iter().filter(|l| st.letters.contains_key(**l)).count(), LETTERS.len())
    }

    /// Pastas de `Animes` que não são letras (preenchido por [`AnimeIndex::root`]).
    pub fn collections(&self) -> Vec<String> {
        self.state.lock().unwrap().collections.clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        if !paused {
            self.resume.notify_waiters();
        }
    }

    pub(crate) fn store(&self, letter: &str, entries: Vec<IndexEntry>) {
        let stored = {
            let mut st = self.state.lock().unwrap();
            if now().saturating_sub(st.at) >= TTL_SECS {
                st.letters.clear();
                st.at = now();
            }
            st.letters.insert(letter.to_owned(), Arc::new(entries));
            Stored { at: st.at, letters: st.letters.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect() }
        };
        if let Ok(b) = serde_json::to_vec(&stored) {
            let _ = std::fs::write(&self.file, b);
        }
    }

    /// Reescreve o arquivo com outro `at` (teste do vencimento).
    #[cfg(test)]
    pub(crate) fn write_at(file: &std::path::Path, at: u64) {
        if let Some(mut s) = std::fs::read(file).ok().and_then(|b| serde_json::from_slice::<Stored>(&b).ok()) {
            s.at = at;
            if let Ok(b) = serde_json::to_vec(&s) {
                let _ = std::fs::write(file, b);
            }
        }
    }

    /// Lista uma pasta; em 429 espera o recuo e repete (até 6 tentativas).
    async fn list_dirs(&self, cloud: &Cloud, path: &str, last: &mut Option<Instant>) -> Result<Vec<String>, String> {
        let mut attempt = 0;
        loop {
            let wait = remaining(*last, Instant::now(), SPACING);
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            let result = cloud.list(path).await;
            *last = Some(Instant::now());
            match result {
                Ok(l) => {
                    let mut names: Vec<String> = l.files.into_iter().filter(|f| f.is_directory).map(|f| f.name).collect();
                    names.sort_by(|a, b| natural_cmp(a, b));
                    return Ok(names);
                }
                Err(HttpError::Status { status: 429, .. }) if attempt + 1 < MAX_ATTEMPTS => {
                    tokio::time::sleep(backoff(attempt)).await;
                    attempt += 1;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    /// Obras da letra: do cache ou de uma listagem. Letra que falha não é guardada.
    pub async fn letter(&self, cloud: &Cloud, letter: &str) -> Result<Arc<Vec<IndexEntry>>, String> {
        if let Some(hit) = self.loaded(letter) {
            return Ok(hit);
        }
        let mut last = self.net.lock().await;
        if let Some(hit) = self.loaded(letter) {
            return Ok(hit);
        }
        let dir = join_path(ROOT, letter);
        let names = self.list_dirs(cloud, &dir, &mut last).await?;
        let entries = names
            .into_iter()
            .map(|name| IndexEntry { letter: letter.to_owned(), path: join_path(&dir, &name), name })
            .collect();
        self.store(letter, entries);
        Ok(self.loaded(letter).unwrap_or_default())
    }

    /// Lista a raiz `Animes`; pastas que não são letras viram coleções.
    pub async fn root(&self, cloud: &Cloud) -> Result<(), String> {
        let mut last = self.net.lock().await;
        if !self.state.lock().unwrap().collections.is_empty() {
            return Ok(());
        }
        let names = self.list_dirs(cloud, ROOT, &mut last).await?;
        self.state.lock().unwrap().collections = names.into_iter().filter(|n| !LETTERS.contains(&n.as_str())).collect();
        Ok(())
    }

    async fn wait_unpaused(&self) {
        loop {
            let notified = self.resume.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.paused.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }

    /// Indexa todas as letras em ordem. A pausa só vale entre letras.
    /// Para no primeiro erro (depois dos recuos); uma nova chamada continua dali.
    pub async fn fill_all(&self, cloud: &Cloud) -> Result<(), String> {
        for letter in LETTERS {
            self.wait_unpaused().await;
            if self.loaded(letter).is_some() {
                continue;
            }
            self.letter(cloud, letter).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(0), Duration::from_secs(5));
        assert_eq!(backoff(1), Duration::from_secs(10));
        assert_eq!(backoff(4), Duration::from_secs(60));
        assert_eq!(backoff(9), Duration::from_secs(60));
    }

    #[test]
    fn remaining_spacing() {
        let t = Instant::now();
        assert_eq!(remaining(None, t, SPACING), Duration::ZERO);
        assert_eq!(remaining(Some(t), t + Duration::from_millis(100), SPACING), Duration::from_millis(300));
        assert_eq!(remaining(Some(t), t + Duration::from_millis(500), SPACING), Duration::ZERO);
    }

    #[tokio::test]
    async fn pause_blocks_until_resumed() {
        let idx = Arc::new(AnimeIndex::open(std::env::temp_dir().join(format!("idx-pause-{}.json", std::process::id()))));
        idx.set_paused(true);
        let task = tokio::spawn({
            let idx = idx.clone();
            async move { idx.wait_unpaused().await }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!task.is_finished());
        idx.set_paused(false);
        tokio::time::timeout(Duration::from_secs(2), task).await.unwrap().unwrap();
    }

    #[test]
    fn letters_in_order() {
        assert_eq!(LETTERS[0], "0-9");
        assert_eq!(LETTERS[1], "Letra A");
        assert_eq!(LETTERS[26], "Letra Z");
    }

    #[test]
    fn persists_and_expires() {
        let dir = std::env::temp_dir().join(format!("idx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("anime-index.json");
        let idx = AnimeIndex::open(file.clone());
        idx.store("Letra P", vec![IndexEntry { letter: "Letra P".into(), name: "Precure".into(), path: "Animes/Letra P/Precure".into() }]);
        assert_eq!(AnimeIndex::open(file.clone()).loaded("Letra P").unwrap()[0].name, "Precure");
        AnimeIndex::write_at(&file, 0); // força `at` antigo
        assert!(AnimeIndex::open(file).loaded("Letra P").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
