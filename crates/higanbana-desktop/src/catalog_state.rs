//! Estado do catálogo de Animes: índice de letras, metadados em segundo plano
//! e pausa enquanto há vídeo tocando.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::{Context, Task};
use higanbana_core::{
    anilist::{MediaType, types::MediaSummary},
    anime_index::{IndexEntry, LETTERS},
};

use crate::state::AppState;

/// Nomes enfileirados em `want_lookup` a cada rodada.
const CRAWL_BATCH: usize = 40;
const CRAWL_EVERY: Duration = Duration::from_secs(3);
const PROGRESS_EVERY: Duration = Duration::from_millis(500);
/// Espera antes de retomar a indexação depois de uma falha.
const INDEX_RETRY: Duration = Duration::from_secs(30);
/// Espera antes de pedir de novo uma letra que falhou.
const LETTER_COOLDOWN: Duration = Duration::from_secs(30);

/// Campos do catálogo dentro de `AppState`.
#[derive(Default)]
pub struct CatalogState {
    index_task: Option<Task<()>>,
    crawl_task: Option<Task<()>>,
    letter_tasks: HashMap<String, Task<()>>,
    letter_failed: HashMap<String, (Instant, String)>,
    crawled: HashSet<String>,
    pending: Vec<String>,
    playing: bool,
    inner_works: HashMap<String, Vec<String>>,
}

/// Tira até `max` nomes do começo, em ordem; o que saiu não volta.
pub fn next_crawl_batch(pending: &mut Vec<String>, max: usize) -> Vec<String> {
    let n = pending.len().min(max);
    pending.drain(..n).collect()
}

/// Uma letra que falhou só é pedida de novo depois do período de espera.
pub fn may_retry(failed_at: Option<Instant>, now: Instant, cooldown: Duration) -> bool {
    failed_at.is_none_or(|at| now.saturating_duration_since(at) >= cooldown)
}

impl AppState {
    /// Idempotente: lista a raiz e indexa as letras no runtime do core.
    pub fn start_anime_index(&mut self, cx: &mut Context<Self>) {
        if self.cat.index_task.is_some() {
            return;
        }
        let core = self.core.clone();
        self.cat.index_task = Some(cx.spawn(async move |this, cx| {
            let mut shown = core.anime_index().progress();
            loop {
                let c = core.clone();
                let task = core.spawn(async move {
                    c.anime_index().root(c.cloud()).await?;
                    c.anime_index().fill_all(c.cloud()).await
                });
                while !task.is_finished() {
                    cx.background_executor().timer(PROGRESS_EVERY).await;
                    let now = core.anime_index().progress();
                    let changed = now != shown;
                    if this.update(cx, |s, cx| if changed && !s.cat.playing { cx.notify() }).is_err() {
                        return;
                    }
                    if changed {
                        shown = now;
                    }
                }
                let (done, total) = core.anime_index().progress();
                let finished = matches!(task.await, Ok(Ok(()))) && done == total;
                let changed = (done, total) != shown;
                shown = (done, total);
                if this.update(cx, |_, cx| if changed { cx.notify() }).is_err() || finished {
                    return;
                }
                cx.background_executor().timer(INDEX_RETRY).await;
            }
        }));
    }

    pub fn anime_letter(&self, letter: &str) -> Option<Arc<Vec<IndexEntry>>> {
        self.core.anime_index().loaded(letter)
    }

    /// Prioriza uma letra ainda não indexada. Depois de uma falha, só tenta de novo após o período de espera.
    pub fn want_anime_letter(&mut self, letter: &str, cx: &mut Context<Self>) {
        if self.anime_letter(letter).is_some() || self.cat.letter_tasks.contains_key(letter) {
            return;
        }
        let failed_at = self.cat.letter_failed.get(letter).map(|(at, _)| *at);
        if !may_retry(failed_at, Instant::now(), LETTER_COOLDOWN) {
            return;
        }
        self.cat.letter_failed.remove(letter);
        let core = self.core.clone();
        let name = letter.to_owned();
        self.cat.letter_tasks.insert(
            name.clone(),
            cx.spawn(async move |this, cx| {
                let c = core.clone();
                let n = name.clone();
                let result = match core.spawn(async move { c.anime_index().letter(c.cloud(), &n).await }).await {
                    Ok(r) => r.map(|_| ()),
                    Err(e) => Err(e.to_string()),
                };
                let _ = this.update(cx, |s, cx| {
                    s.cat.letter_tasks.remove(&name);
                    // A falha fica guardada: o aviso aparece, mas o novo pedido espera o período.
                    if let Err(e) = result {
                        s.cat.letter_failed.insert(name, (Instant::now(), e));
                    }
                    cx.notify();
                });
            }),
        );
    }

    /// Último erro ao carregar a letra, enquanto estiver no período de espera.
    pub fn anime_letter_error(&self, letter: &str) -> Option<String> {
        self.cat.letter_failed.get(letter).map(|(_, e)| e.clone())
    }

    pub fn anime_collections(&self) -> Vec<String> {
        self.core.anime_index().collections()
    }

    pub fn index_progress(&self) -> (usize, usize) {
        self.core.anime_index().progress()
    }

    /// Enfileira aos poucos os nomes das letras que chegam (40 a cada 3 s); não roda com vídeo.
    pub fn start_metadata_crawl(&mut self, cx: &mut Context<Self>) {
        if self.cat.crawl_task.is_some() {
            return;
        }
        self.cat.crawl_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CRAWL_EVERY).await;
                let Ok(done) = this.update(cx, |s, cx| s.crawl_step(cx)) else { return };
                if done {
                    return;
                }
            }
        }));
    }

    /// Uma rodada; `true` quando não há mais nada a fazer.
    fn crawl_step(&mut self, cx: &mut Context<Self>) -> bool {
        if self.cat.playing || self.lookups_busy() {
            return false;
        }
        for letter in LETTERS {
            if self.cat.crawled.contains(letter) {
                continue;
            }
            if let Some(entries) = self.anime_letter(letter) {
                self.cat.crawled.insert(letter.to_owned());
                self.cat.pending.extend(entries.iter().map(|e| e.name.clone()));
            }
        }
        for name in next_crawl_batch(&mut self.cat.pending, CRAWL_BATCH) {
            self.want_lookup(&name, MediaType::Anime, cx);
        }
        self.cat.pending.is_empty() && self.cat.crawled.len() == LETTERS.len()
    }

    /// Nomes resolvidos / total de nomes no índice.
    pub fn metadata_progress(&self) -> (usize, usize) {
        let mut total = 0;
        let mut resolved = 0;
        for letter in LETTERS {
            if let Some(entries) = self.anime_letter(letter) {
                total += entries.len();
                resolved += entries.iter().filter(|e| self.lookup(&e.name, MediaType::Anime).is_some()).count();
            }
        }
        (resolved, total)
    }

    pub fn set_playing(&mut self, playing: bool, _cx: &mut Context<Self>) {
        self.cat.playing = playing;
        self.core.anime_index().set_paused(playing);
    }

    pub fn set_inner_works(&mut self, title_path: &str, names: Vec<String>) {
        self.cat.inner_works.insert(title_path.to_owned(), names);
    }

    pub fn inner_summaries(&self, title_path: &str) -> Vec<MediaSummary> {
        self.cat
            .inner_works
            .get(title_path)
            .map(|names| names.iter().filter_map(|n| self.lookup(n, MediaType::Anime).flatten()).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_cooldown() {
        let t = Instant::now();
        let cd = Duration::from_secs(30);
        assert!(may_retry(None, t, cd));
        assert!(!may_retry(Some(t), t + Duration::from_secs(29), cd));
        assert!(may_retry(Some(t), t + Duration::from_secs(30), cd));
    }

    #[test]
    fn crawl_batches() {
        let mut pending: Vec<String> = (0..5).map(|i| format!("n{i}")).collect();
        assert_eq!(next_crawl_batch(&mut pending, 2), vec!["n0", "n1"]);
        assert_eq!(next_crawl_batch(&mut pending, 2), vec!["n2", "n3"]);
        assert_eq!(next_crawl_batch(&mut pending, 2), vec!["n4"]);
        assert!(next_crawl_batch(&mut pending, 2).is_empty());
    }
}
