//! Estado dos catálogos de séries (Animes, Mangás): índice de letras, metadados em
//! segundo plano e pausa enquanto há vídeo tocando. Uma faixa por mídia.
//! As rodadas são `after` encadeados no runtime do core (sem timer quando parado).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use higanbana_core::anilist::{types::MediaSummary, MediaType, Priority};
use higanbana_core::anime_index::{IndexEntry, LETTERS};
use higanbana_core::catalog_filter::{Filters, Sort};
use higanbana_core::library::CATALOG_MEDIA;

use super::AppState;

/// Nomes enfileirados a cada rodada: um lote, uma requisição do AniList.
const CRAWL_BATCH: usize = 18;
const CRAWL_EVERY: Duration = Duration::from_secs(1);
const PROGRESS_EVERY: Duration = Duration::from_millis(500);
/// Espera antes de retomar a indexação depois de uma falha.
const INDEX_RETRY: Duration = Duration::from_secs(30);
/// Espera antes de pedir de novo uma letra que falhou.
const LETTER_COOLDOWN: Duration = Duration::from_secs(30);
/// Espera depois da última tecla antes de buscar na nuvem.
const CLOUD_DEBOUNCE: Duration = Duration::from_millis(400);
/// Busca na nuvem só a partir deste tamanho.
const CLOUD_MIN_CHARS: usize = 3;

/// Busca na nuvem do catálogo (obras dentro de franquias). `geracao` descarta respostas velhas.
#[derive(Default, Clone)]
pub struct CloudSearch {
    pub query: String,
    pub results: Vec<higanbana_core::nuvem::SearchResult>,
    pub failed: bool,
    geracao: u64,
}

impl CloudSearch {
    /// Novo texto: `Some(geração)` se precisa buscar (depois da espera).
    fn ask(&mut self, q: &str) -> Option<u64> {
        if self.query == q {
            return None;
        }
        self.query = q.to_owned();
        self.results.clear();
        self.failed = false;
        self.geracao += 1;
        (q.chars().count() >= CLOUD_MIN_CHARS).then_some(self.geracao)
    }

    /// Resposta da geração `geracao`; `false` se já é velha.
    fn answer(&mut self, geracao: u64, r: Option<Vec<higanbana_core::nuvem::SearchResult>>) -> bool {
        if geracao != self.geracao {
            return false;
        }
        self.failed = r.is_none();
        self.results = r.unwrap_or_default();
        true
    }
}

pub struct CatalogState {
    lanes: HashMap<MediaType, Lane>,
    playing: bool,
    inner_works: HashMap<String, Vec<String>>,
    inner_gen: u64,
    cloud: HashMap<MediaType, CloudSearch>,
}

impl Default for CatalogState {
    fn default() -> Self {
        Self { lanes: CATALOG_MEDIA.into_iter().map(|m| (m, Lane::default())).collect(), playing: false, inner_works: HashMap::new(), inner_gen: 0, cloud: HashMap::new() }
    }
}

#[derive(Default)]
struct Lane {
    indexing: bool,
    /// (progresso, nuvem em pausa) da última vez que a tela foi avisada.
    shown: Option<((usize, usize), bool)>,
    crawling: bool,
    crawl_paused: bool,
    letters_loading: HashSet<String>,
    letter_failed: HashMap<String, (Instant, String)>,
    crawled: HashSet<String>,
    pending: Vec<String>,
    crawl_sent: HashSet<String>,
    meta_progress: std::cell::Cell<Option<((usize, u64), (usize, usize))>>,
    view: CatalogChoice,
}

/// O que a pessoa escolheu na tela do catálogo e onde a tela estava ao sair.
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
    /// Letra no topo da tela; `None` = cabeçalho.
    pub top: Option<usize>,
    /// Quanto a tela estava rolada, em pixels.
    pub offset: f32,
}

/// O rastreio só trabalha quando a tela precisa dos metadados de todas as obras.
pub fn crawl_wanted(choice: &CatalogChoice) -> bool {
    choice.filters.needs_metadata() || choice.sort != Sort::Az
}

/// Próxima letra a indexar para o rastreio (uma por rodada, em ordem). Só quando a tela
/// precisa de todas as obras (filtro/ordenação); no A–Z as letras vêm do scroll.
pub fn next_letter_to_index(crawl: bool, loaded: impl Fn(&str) -> bool) -> Option<&'static str> {
    if !crawl {
        return None;
    }
    LETTERS.into_iter().find(|l| !loaded(l))
}

/// Tira até `max` nomes do começo, em ordem.
pub fn next_crawl_batch(pending: &mut Vec<String>, max: usize) -> Vec<String> {
    let n = pending.len().min(max);
    pending.drain(..n).collect()
}

/// Um lote voltou: os nomes do rastreio saem de `sent`; se falhou, voltam ao começo da fila.
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
    fn cat_lane(&self, media: MediaType) -> &Lane {
        &self.catalog.lanes[&media]
    }

    fn cat_lane_mut(&mut self, media: MediaType) -> &mut Lane {
        self.catalog.lanes.entry(media).or_default()
    }

    fn index_look(&self, media: MediaType) -> ((usize, usize), bool) {
        (self.core.section_index(media).progress(), self.core.cloud().backing_off_until().is_some())
    }

    /// Idempotente: lista a raiz e indexa as letras no runtime do core.
    pub fn start_index(&mut self, media: MediaType) {
        if self.cat_lane(media).indexing {
            return;
        }
        self.cat_lane_mut(media).indexing = true;
        let core = self.core.clone();
        // Só a raiz (coleções). As letras vêm do scroll (`want_catalog_letter`) ou, com
        // filtro/ordenação que precisa de tudo, do rastreio, uma por rodada: listar as 27
        // de uma vez ao abrir a tela era uma varredura da nuvem.
        self.run(
            async move { core.section_index(media).root(core.cloud()).await },
            move |s, r, cx| {
                let finished = r.is_ok();
                s.cat_lane_mut(media).indexing = false;
                if !s.catalog.playing {
                    cx.notify();
                }
                if !finished {
                    s.after(INDEX_RETRY, move |s, _| s.start_index(media));
                }
            },
        );
        self.poll_index(media);
    }

    /// Enquanto indexa: avisa a tela quando o progresso ou a pausa da nuvem mudam.
    fn poll_index(&mut self, media: MediaType) {
        self.after(PROGRESS_EVERY, move |s, cx| {
            let now = s.index_look(media);
            let playing = s.catalog.playing;
            let lane = s.cat_lane_mut(media);
            if lane.shown != Some(now) {
                lane.shown = Some(now);
                if !playing {
                    cx.notify();
                }
            }
            if s.cat_lane(media).indexing {
                s.poll_index(media);
            }
        });
    }

    pub fn catalog_letter(&self, media: MediaType, letter: &str) -> Option<Arc<Vec<IndexEntry>>> {
        self.core.section_index(media).loaded(letter)
    }

    /// Prioriza uma letra ainda não indexada (depois de falha, só após a espera).
    /// `true` se começou um pedido agora.
    pub fn want_catalog_letter(&mut self, media: MediaType, letter: &str) -> bool {
        if self.catalog_letter(media, letter).is_some() || self.cat_lane(media).letters_loading.contains(letter) {
            return false;
        }
        let failed_at = self.cat_lane(media).letter_failed.get(letter).map(|(at, _)| *at);
        if !may_retry(failed_at, Instant::now(), LETTER_COOLDOWN) {
            return false;
        }
        let lane = self.cat_lane_mut(media);
        lane.letter_failed.remove(letter);
        lane.letters_loading.insert(letter.to_owned());
        let (core, name) = (self.core.clone(), letter.to_owned());
        let key = name.clone();
        self.run(async move { core.section_index(media).letter(core.cloud(), &name).await.map(|_| ()) }, move |s, r, cx| {
            let lane = s.cat_lane_mut(media);
            lane.letters_loading.remove(&key);
            if let Err(e) = r {
                lane.letter_failed.insert(key, (Instant::now(), e.to_string()));
            }
            cx.notify();
        });
        true
    }

    pub fn catalog_letter_error(&self, media: MediaType, letter: &str) -> Option<String> {
        self.cat_lane(media).letter_failed.get(letter).map(|(_, e)| e.clone())
    }

    pub fn catalog_collections(&self, media: MediaType) -> Vec<String> {
        self.core.section_index(media).collections()
    }

    pub fn index_progress(&self, media: MediaType) -> (usize, usize) {
        self.core.section_index(media).progress()
    }

    /// Até quando a nuvem pediu pausa (HTTP 429).
    pub fn cloud_paused_until(&self) -> Option<Instant> {
        self.core.cloud().backing_off_until()
    }

    /// Enfileira aos poucos os nomes das letras (um lote por rodada); não roda com vídeo.
    pub fn start_metadata_crawl(&mut self, media: MediaType) {
        if self.cat_lane(media).crawling {
            return;
        }
        self.cat_lane_mut(media).crawling = true;
        self.crawl_tick(media);
    }

    fn crawl_tick(&mut self, media: MediaType) {
        self.after(CRAWL_EVERY, move |s, cx| {
            // A pausa do AniList começou ou acabou: redesenha para o aviso aparecer.
            let paused = s.anilist_paused_until().is_some();
            if paused != s.cat_lane(media).crawl_paused && !s.catalog.playing {
                s.cat_lane_mut(media).crawl_paused = paused;
                cx.notify();
            }
            if s.crawl_step(media) {
                s.cat_lane_mut(media).crawling = false;
            } else {
                s.crawl_tick(media);
            }
        });
    }

    /// Uma rodada; `true` quando não há mais nada a fazer.
    fn crawl_step(&mut self, media: MediaType) -> bool {
        if !crawl_wanted(&self.cat_lane(media).view) || self.catalog.playing || self.lookups_busy() {
            return false;
        }
        if let Some(letter) = next_letter_to_index(true, |l| self.catalog_letter(media, l).is_some()) {
            self.want_catalog_letter(media, letter);
        }
        for letter in LETTERS {
            if self.cat_lane(media).crawled.contains(letter) {
                continue;
            }
            if let Some(entries) = self.catalog_letter(media, letter) {
                let lane = self.cat_lane_mut(media);
                lane.crawled.insert(letter.to_owned());
                lane.pending.extend(entries.iter().map(|e| e.name.clone()));
            }
        }
        for name in next_crawl_batch(&mut self.cat_lane_mut(media).pending, CRAWL_BATCH) {
            if self.want_lookup_with(&name, media, Priority::Background) {
                self.cat_lane_mut(media).crawl_sent.insert(name);
            }
        }
        let lane = self.cat_lane(media);
        lane.pending.is_empty() && lane.crawl_sent.is_empty() && lane.crawled.len() == LETTERS.len()
    }

    /// Resposta de um lote: nomes do rastreio num lote que falhou voltam à fila.
    pub(crate) fn crawl_lookups_done(&mut self, keys: &[(MediaType, String)], failed: bool) {
        for media in CATALOG_MEDIA {
            let names: Vec<String> = keys.iter().filter(|(m, _)| *m == media).map(|(_, n)| n.clone()).collect();
            if !names.is_empty() {
                let lane = self.cat_lane_mut(media);
                settle_crawl(&mut lane.pending, &mut lane.crawl_sent, &names, failed);
            }
        }
    }

    /// Nomes resolvidos / total no índice. Recontado só quando chega letra ou resposta.
    pub fn metadata_progress(&self, media: MediaType) -> (usize, usize) {
        let loaded = LETTERS.iter().filter(|l| self.catalog_letter(media, l).is_some()).count();
        let key = (loaded, self.lookups_gen());
        let cache = &self.cat_lane(media).meta_progress;
        if let Some((k, progress)) = cache.get()
            && k == key
        {
            return progress;
        }
        let letters: Vec<Arc<Vec<IndexEntry>>> = LETTERS.iter().filter_map(|l| self.catalog_letter(media, l)).collect();
        let total = letters.iter().map(|l| l.len()).sum();
        let resolved = letters.iter().flat_map(|l| l.iter()).filter(|e| self.lookup_ref(&e.name, media).is_some()).count();
        cache.set(Some((key, (resolved, total))));
        (resolved, total)
    }

    /// Player aberto: solta imagens e pausa índice/rastreio.
    pub fn set_playing(&mut self, playing: bool) {
        self.catalog.playing = playing;
        if playing {
            self.release_all_images();
        }
        self.core.set_index_paused(playing);
    }

    /// Como `set_inner_works`, só quando mudou (chamado a cada render da franquia).
    pub fn set_inner_works_if_changed(&mut self, title_path: &str, names: &[String]) {
        if self.catalog.inner_works.get(title_path).is_none_or(|n| n.as_slice() != names) {
            self.set_inner_works(title_path, names.to_vec());
        }
    }

    pub fn set_inner_works(&mut self, title_path: &str, names: Vec<String>) {
        self.catalog.inner_works.insert(title_path.to_owned(), names);
        self.catalog.inner_gen += 1;
    }

    pub fn inner_works_gen(&self) -> u64 {
        self.catalog.inner_gen
    }

    pub fn catalog_choice(&self, media: MediaType) -> &CatalogChoice {
        &self.cat_lane(media).view
    }

    /// Guarda os filtros e a ordenação (a busca fica como está).
    pub fn save_catalog_filters(&mut self, media: MediaType, filters: &Filters, sort: Sort) {
        let view = &mut self.cat_lane_mut(media).view;
        let query = std::mem::take(&mut view.filters.query);
        *view = CatalogChoice { filters: Filters { query, ..filters.clone() }, sort, place: view.place };
    }

    pub fn save_catalog_query(&mut self, media: MediaType, query: &str) {
        query.clone_into(&mut self.cat_lane_mut(media).view.filters.query);
    }

    pub fn save_catalog_place(&mut self, media: MediaType, place: CatalogPlace) {
        self.cat_lane_mut(media).view.place = place;
    }

    /// Busca na nuvem por `q` (só a partir de 3 letras, depois de 400 ms sem digitar).
    pub fn catalog_cloud_search(&mut self, media: MediaType, q: &str) {
        let Some(geracao) = self.catalog.cloud.entry(media).or_default().ask(q) else { return };
        let q = q.to_owned();
        self.after(CLOUD_DEBOUNCE, move |s, _| {
            if s.catalog.cloud.get(&media).is_none_or(|c| c.geracao != geracao) {
                return;
            }
            let core = s.core.clone();
            let asked = q.clone();
            s.run(async move { core.cloud().search(&asked).await }, move |s, r, cx| {
                let r = match r {
                    Ok(r) => Some(r.results),
                    Err(e) => {
                        tracing::warn!(query = %q, error = %e, "busca na nuvem falhou");
                        None
                    }
                };
                if s.catalog.cloud.entry(media).or_default().answer(geracao, r) {
                    cx.notify();
                }
            });
        });
    }

    pub fn catalog_cloud(&self, media: MediaType) -> Option<&CloudSearch> {
        self.catalog.cloud.get(&media)
    }

    pub fn inner_summaries(&self, media: MediaType, title_path: &str) -> Vec<MediaSummary> {
        self.catalog.inner_works.get(title_path).map(|names| names.iter().filter_map(|n| self.lookup_ref(n, media).flatten().cloned()).collect()).unwrap_or_default()
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
        settle_crawl(&mut pending, &mut sent, &names(&["a", "x"]), true);
        assert_eq!(pending, names(&["a", "c"]));
        assert_eq!(sent, names(&["b"]).into_iter().collect());
        settle_crawl(&mut pending, &mut sent, &names(&["b"]), false);
        assert_eq!(pending, names(&["a", "c"]));
        assert!(sent.is_empty());
    }

    #[test]
    fn indexa_uma_letra_por_vez_so_quando_o_filtro_pede() {
        let loaded = |set: &'static [&'static str]| move |l: &str| set.contains(&l);
        // Catálogo em A–Z: as letras vêm do scroll, nada de varrer a nuvem.
        assert_eq!(next_letter_to_index(false, loaded(&[])), None);
        assert_eq!(next_letter_to_index(true, loaded(&[])), Some("0-9"));
        assert_eq!(next_letter_to_index(true, loaded(&["0-9", "Letra A"])), Some("Letra B"));
        assert_eq!(next_letter_to_index(true, |_| true), None);
    }

    #[test]
    fn crawl_only_when_metadata_is_needed() {
        let choice = |filters: Filters, sort: Sort| CatalogChoice { filters, sort, place: CatalogPlace::default() };
        assert!(!crawl_wanted(&CatalogChoice::default()));
        assert!(!crawl_wanted(&choice(Filters { query: "naruto".into(), dubbed: true, ..Default::default() }, Sort::Az)));
        assert!(crawl_wanted(&choice(Filters { genres: vec!["Comedy".into()], ..Default::default() }, Sort::Az)));
        assert!(crawl_wanted(&choice(Filters { years: Some((2009, 2011)), ..Default::default() }, Sort::Az)));
        assert!(crawl_wanted(&choice(Filters::default(), Sort::Year)));
        assert!(crawl_wanted(&choice(Filters::default(), Sort::Score)));
    }

    #[test]
    fn busca_velha_na_nuvem_descartada() {
        let mut c = CloudSearch::default();
        let a = c.ask("frieren").unwrap();
        let b = c.ask("frieren b").unwrap();
        assert!(!c.answer(a, Some(Vec::new())), "resposta do texto antigo");
        assert!(c.answer(b, None));
        assert!(c.failed);
        assert_eq!(c.ask("fr"), None, "curta demais: não busca");
        assert!(c.results.is_empty() && !c.failed);
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
