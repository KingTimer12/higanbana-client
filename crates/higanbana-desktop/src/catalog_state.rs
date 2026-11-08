//! Estado do catálogo de Animes: índice de letras, metadados em segundo plano
//! e pausa enquanto há vídeo tocando.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::{Context, Task};
use higanbana_core::{
    anilist::{MediaType, types::MediaSummary},
    anime_index::{IndexEntry, LETTERS},
    catalog_filter::{Filters, Sort},
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

/// (letras carregadas, geração das respostas) e o `metadata_progress` medido com elas.
type MetaProgress = ((usize, u64), (usize, usize));

/// Campos do catálogo dentro de `AppState`.
#[derive(Default)]
pub struct CatalogState {
    index_task: Option<Task<()>>,
    crawl_task: Option<Task<()>>,
    letter_tasks: HashMap<String, Task<()>>,
    letter_failed: HashMap<String, (Instant, String)>,
    crawled: HashSet<String>,
    pending: Vec<String>,
    /// Nomes que o rastreio pediu e ainda não tiveram resposta.
    crawl_sent: HashSet<String>,
    playing: bool,
    inner_works: HashMap<String, Vec<String>>,
    /// Muda a cada `set_inner_works`.
    inner_gen: u64,
    /// `metadata_progress` em cache: (letras carregadas, geração das respostas) → resultado.
    meta_progress: Cell<Option<MetaProgress>>,
    /// Filtros, ordenação e busca do catálogo: sobrevivem a abrir uma obra e voltar.
    view: CatalogChoice,
}

/// O que a pessoa escolheu na tela do catálogo (`filters.query` é o texto da busca)
/// e onde a tela estava ao sair.
#[derive(Clone)]
pub struct CatalogChoice {
    pub filters: Filters,
    pub sort: Sort,
    pub place: CatalogPlace,
}

impl Default for CatalogChoice {
    fn default() -> Self {
        Self { filters: Filters::default(), sort: Sort::Az, place: CatalogPlace::default() }
    }
}

/// Posição da tela do catálogo, para voltar ao mesmo lugar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CatalogPlace {
    /// Última letra pedida (posição em `LETTERS`).
    pub upto: usize,
    /// Letra no topo da tela (grade por letras, sem busca); `None` = cabeçalho.
    pub top: Option<usize>,
    /// Quanto a tela estava rolada, em pixels (busca ou grade única).
    pub offset: f32,
}

/// Tira até `max` nomes do começo, em ordem; o que saiu não volta.
pub fn next_crawl_batch(pending: &mut Vec<String>, max: usize) -> Vec<String> {
    let n = pending.len().min(max);
    pending.drain(..n).collect()
}

/// Um lote do AniList voltou: os nomes do rastreio saem de `sent`; se o lote
/// falhou, voltam ao começo da fila para serem pedidos de novo.
pub fn settle_crawl(pending: &mut Vec<String>, sent: &mut HashSet<String>, names: &[String], failed: bool) {
    let mine: Vec<String> = names.iter().filter(|n| sent.remove(n.as_str())).cloned().collect();
    if failed {
        pending.splice(0..0, mine);
    }
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
            // Progresso e pausa da nuvem (429): mudou, redesenha.
            let look = |core: &higanbana_core::Core| (core.anime_index().progress(), core.cloud().backing_off_until().is_some());
            let mut shown = look(&core);
            loop {
                let c = core.clone();
                let task = core.spawn(async move {
                    c.anime_index().root(c.cloud()).await?;
                    c.anime_index().fill_all(c.cloud()).await
                });
                while !task.is_finished() {
                    cx.background_executor().timer(PROGRESS_EVERY).await;
                    let now = look(&core);
                    let changed = now != shown;
                    if this.update(cx, |s, cx| if changed && !s.cat.playing { cx.notify() }).is_err() {
                        return;
                    }
                    if changed {
                        shown = now;
                    }
                }
                let now = look(&core);
                let (done, total) = now.0;
                let finished = matches!(task.await, Ok(Ok(()))) && done == total;
                let changed = now != shown;
                shown = now;
                let updated = this.update(cx, |s, cx| {
                    // Terminou: libera a vaga para reabrir o catálogo recomeçar depois do vencimento (12 h).
                    if finished && let Some(task) = s.cat.index_task.take() {
                        task.detach();
                    }
                    if changed {
                        cx.notify();
                    }
                });
                if updated.is_err() || finished {
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

    /// Até quando a nuvem pediu pausa (HTTP 429), se ainda estiver em vigor.
    pub fn cloud_paused_until(&self) -> Option<Instant> {
        self.core.cloud().backing_off_until()
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

    /// Uma rodada; `true` quando não há mais nada a fazer. Espera a resposta
    /// do lote anterior (no máximo um lote do rastreio em andamento).
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
            if self.want_lookup(&name, MediaType::Anime, cx) {
                self.cat.crawl_sent.insert(name);
            }
        }
        self.cat.pending.is_empty() && self.cat.crawl_sent.is_empty() && self.cat.crawled.len() == LETTERS.len()
    }

    /// Resposta de um lote: nomes do rastreio num lote que falhou voltam à fila.
    pub(crate) fn crawl_lookups_done(&mut self, names: &[String], failed: bool) {
        settle_crawl(&mut self.cat.pending, &mut self.cat.crawl_sent, names, failed);
    }

    /// Nomes resolvidos / total de nomes no índice. Recontado só quando chega letra ou resposta.
    pub fn metadata_progress(&self) -> (usize, usize) {
        let letters: Vec<Arc<Vec<IndexEntry>>> = LETTERS.iter().filter_map(|l| self.anime_letter(l)).collect();
        let key = (letters.len(), self.lookups_gen());
        if let Some((k, progress)) = self.cat.meta_progress.get()
            && k == key
        {
            return progress;
        }
        let entries = letters.iter().flat_map(|l| l.iter());
        let total = letters.iter().map(|l| l.len()).sum();
        let resolved = entries.filter(|e| self.lookup_ref(&e.name, MediaType::Anime).is_some()).count();
        self.cat.meta_progress.set(Some((key, (resolved, total))));
        (resolved, total)
    }

    pub fn set_playing(&mut self, playing: bool, _cx: &mut Context<Self>) {
        self.cat.playing = playing;
        self.core.anime_index().set_paused(playing);
    }

    pub fn set_inner_works(&mut self, title_path: &str, names: Vec<String>) {
        self.cat.inner_works.insert(title_path.to_owned(), names);
        self.cat.inner_gen += 1;
    }

    /// Muda sempre que as obras de dentro de alguma franquia mudam.
    pub fn inner_works_gen(&self) -> u64 {
        self.cat.inner_gen
    }

    /// Filtros, ordenação e busca guardados da última vez na tela do catálogo.
    pub fn catalog_choice(&self) -> &CatalogChoice {
        &self.cat.view
    }

    /// Guarda os filtros e a ordenação (a busca fica como está).
    pub fn save_catalog_filters(&mut self, filters: &Filters, sort: Sort) {
        let query = std::mem::take(&mut self.cat.view.filters.query);
        self.cat.view = CatalogChoice { filters: Filters { query, ..filters.clone() }, sort, place: self.cat.view.place };
    }

    pub fn save_catalog_query(&mut self, query: &str) {
        self.cat.view.filters.query = query.to_owned();
    }

    pub fn save_catalog_place(&mut self, place: CatalogPlace) {
        self.cat.view.place = place;
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
    fn failed_crawl_names_return_to_queue() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut pending = names(&["c"]);
        let mut sent: HashSet<String> = names(&["a", "b"]).into_iter().collect();
        // "x" veio de um card visível, não do rastreio: não entra na fila.
        settle_crawl(&mut pending, &mut sent, &names(&["a", "x"]), true);
        assert_eq!(pending, names(&["a", "c"]));
        assert_eq!(sent, names(&["b"]).into_iter().collect());
        // Lote respondido: só sai de `sent`.
        settle_crawl(&mut pending, &mut sent, &names(&["b"]), false);
        assert_eq!(pending, names(&["a", "c"]));
        assert!(sent.is_empty());
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
