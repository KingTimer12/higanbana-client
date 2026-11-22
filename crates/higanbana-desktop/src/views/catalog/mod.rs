//! Catálogo de séries (Animes, Mangás; ver `series::Series`): todas as obras de
//! 0-9 a Z numa lista virtual, índice lateral, busca por nome, filtros e ordenação.
//! Busca, filtros e posição sobrevivem a abrir uma obra e voltar (`AppState`).

mod filters;
mod header;
mod rail;
mod render_row;
mod rows;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::{Duration, Instant};

use higanbana_core::anilist::MediaType;
use higanbana_core::anime_index::{letter_of, IndexEntry, LETTERS};
use higanbana_core::catalog_filter::{Filters, Sort};
use higanbana_gpui::components::{PopoverState, VirtualList};
use higanbana_gpui::prelude::*;

use filters::logic::{empty_notice, filtered_works, franchise_results, year_fields_text, EmptyNotice, Work};
use rows::{flat_rows, letter_at, letter_row, letter_rows, search_rows, search_sections, Body, LetterSection, Metrics, Row};

use crate::root::Higanbana;
use crate::series::Series;
use crate::state::CatalogPlace;
use crate::ui::cover::{columns, content_width};

/// Id do contêiner rolável (lista virtual).
pub const LIST: &str = "catalogo-lista";
const RAIL_W: f32 = 40.0;
const GAP: f32 = 20.0;
/// Pouco mais que a espera de uma letra com falha (30 s).
const RETRY_CHECK: Duration = Duration::from_secs(31);

/// Onde a tela reabre.
#[derive(Debug, PartialEq)]
pub enum Reopen {
    Top,
    Letter(usize),
    Offset(f32),
}

/// A letra da rota vence; sem ela, volta à letra do topo (grade por letras) ou à rolagem salva.
pub fn reopen_at(route: Option<usize>, place: &CatalogPlace, by_letter: bool) -> Reopen {
    match route {
        Some(letter) => Reopen::Letter(letter),
        None if by_letter => place.top.map_or(Reopen::Top, Reopen::Letter),
        None if place.offset > 0. => Reopen::Offset(place.offset),
        None => Reopen::Top,
    }
}

/// Qual menu da barra está aberto.
#[derive(Default)]
pub struct Menus {
    pub kind: PopoverState,
    pub genre: PopoverState,
    pub year: PopoverState,
    pub season: PopoverState,
    pub sort: PopoverState,
    pub collections: PopoverState,
}

/// O que define a grade única: igual, a lista em cache vale.
#[derive(PartialEq)]
struct FlatKey {
    filters: Filters,
    sort: Sort,
    indexed: usize,
    resolved: usize,
    inner_gen: u64,
}

pub struct CatalogUi {
    pub series: Series,
    pub search: TextInputState,
    pub year_from: TextInputState,
    pub year_to: TextInputState,
    pub filters: Filters,
    pub sort: Sort,
    pub menus: Menus,
    /// Última letra pedida (sem busca, as seções vão de 0-9 até ela).
    pub upto: usize,
    /// Letra pedida pela rota (`Animes/Letra F`).
    route_letter: Option<usize>,
    restored: bool,
    /// Índice e rastreio já pedidos nesta abertura (pedir a cada render recomeçaria o índice em laço).
    started: bool,
    flat: Option<(FlatKey, Arc<[IndexEntry]>)>,
    found: Option<(String, usize, Vec<LetterSection>)>,
    pub genres: Option<((usize, usize), Vec<String>)>,
}

impl CatalogUi {
    pub fn new(saved: &crate::state::CatalogChoice, media: MediaType, letter: Option<String>) -> Self {
        let saved = saved.clone();
        let (from, to) = year_fields_text(saved.filters.years);
        let route_letter = letter.and_then(|l| LETTERS.iter().position(|x| *x == l));
        CatalogUi {
            series: Series::of(media),
            search: TextInputState::with_text(saved.filters.query.clone()).placeholder("Buscar no catálogo"),
            year_from: TextInputState::with_text(from).placeholder("De"),
            year_to: TextInputState::with_text(to).placeholder("Até"),
            filters: Filters { query: String::new(), ..saved.filters },
            sort: saved.sort,
            menus: Menus::default(),
            upto: if route_letter.is_some() { 0 } else { saved.place.upto.min(LETTERS.len() - 1) },
            route_letter,
            restored: false,
            started: false,
            flat: None,
            found: None,
            genres: None,
        }
    }

    pub fn query(&self) -> String {
        self.search.text().trim().to_owned()
    }

    /// Com filtro ou fora de A–Z a tela é uma grade só, sem cabeçalhos de letra.
    pub fn flat_mode(&self) -> bool {
        !self.filters.is_empty() || self.sort != Sort::Az
    }
}

fn ui(v: &mut Higanbana) -> &mut CatalogUi {
    let media = v.pages.catalog_media();
    v.pages.catalog(&v.state, media)
}

/// Busca ou filtros mudaram: guarda, volta ao topo e pede a busca na nuvem.
pub fn restart(v: &mut Higanbana, cx: &mut EventCx) {
    let c = ui(v);
    let (media, query, filters, sort) = (c.series.media, c.query(), c.filters.clone(), c.sort);
    v.state.save_catalog_query(media, &query);
    v.state.save_catalog_filters(media, &filters, sort);
    v.state.catalog_cloud_search(media, &query);
    // A busca local só vê letras já listadas: pede a da inicial (uma listagem, não todas).
    if let Some(l) = letter_of(&query) {
        v.state.want_catalog_letter(media, l);
    }
    cx.scroll_to(LIST, 0.0);
    cx.notify();
}

/// Rola até a letra; sem busca, também passa a desenhar até ela e a pede.
pub fn jump(v: &mut Higanbana, letter: usize, offset: f32, cx: &mut EventCx) {
    let c = ui(v);
    let searching = !c.query().is_empty();
    let media = c.series.media;
    if !searching {
        c.upto = c.upto.max(letter);
        v.state.want_catalog_letter(media, LETTERS[letter]);
    }
    cx.scroll_to(LIST, offset);
    cx.notify();
}

fn flat_works(v: &mut Higanbana, resolved: usize) -> Arc<[IndexEntry]> {
    let media = v.pages.catalog_media();
    let (indexed, inner_gen) = (v.state.index_progress(media).0, v.state.inner_works_gen());
    let c = v.pages.catalog(&v.state, media);
    let query = c.query();
    let key = FlatKey { filters: Filters { query: query.clone(), ..c.filters.clone() }, sort: c.sort, indexed, resolved, inner_gen };
    if let Some((k, works)) = &c.flat
        && *k == key
    {
        return works.clone();
    }
    let st = &v.state;
    let letters: Vec<Arc<Vec<IndexEntry>>> = LETTERS.iter().filter_map(|l| st.catalog_letter(media, l)).collect();
    let works: Vec<Work> = letters.iter().flat_map(|l| l.iter()).map(|e| Work { entry: e, meta: st.lookup_ref(&e.name, media).flatten(), inner: st.inner_summaries(media, &e.path) }).collect();
    let found: Arc<[IndexEntry]> = filtered_works(&works, &key.filters, key.sort).into();
    let c = v.pages.catalog(&v.state, media);
    c.flat = Some((key, found.clone()));
    found
}

fn found_sections(v: &mut Higanbana, query: &str) -> Vec<LetterSection> {
    let media = v.pages.catalog_media();
    let indexed = v.state.index_progress(media).0;
    let fresh = v.pages.catalog(&v.state, media).found.as_ref().is_some_and(|(q, n, _)| q == query && *n == indexed);
    if !fresh {
        let loaded: Vec<(usize, Arc<Vec<IndexEntry>>)> = LETTERS.iter().enumerate().filter_map(|(ix, l)| v.state.catalog_letter(media, l).map(|e| (ix, e))).collect();
        let found = search_sections(loaded.iter().map(|(ix, e)| (*ix, e.as_slice())), query);
        v.pages.catalog(&v.state, media).found = Some((query.to_owned(), indexed, found));
    }
    v.pages.catalog(&v.state, media).found.as_ref().map(|f| f.2.clone()).unwrap_or_default()
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let media = v.pages.catalog_media();
    if !std::mem::replace(&mut v.pages.catalog(&v.state, media).started, true) {
        v.state.start_index(media);
        v.state.start_metadata_crawl(media);
    }
    let width = (content_width(v, cx, 1280.0 + RAIL_W) - RAIL_W).max(200.0);
    let cols = columns(width, 160.0, GAP);
    let card_w = ((width - (cols as f32 - 1.0) * GAP) / cols as f32).floor();
    let header_h = cx.bounds("catalogo-cabecalho").map_or(220.0, |r| r.size.height);
    let m = Metrics { cols, card_w, header_h };
    let c = v.pages.catalog(&v.state, media);
    let (query, flat_mode, upto) = (c.query(), c.flat_mode(), c.upto);
    let franchise: Vec<_> = v.state.catalog_cloud(media).filter(|s| s.query == query).map(|s| franchise_results(s.results.clone(), Series::of(media))).unwrap_or_default();
    let mut flat_status = None;
    let rows = if flat_mode {
        let metadata = v.state.metadata_progress(media);
        let works = flat_works(v, metadata.0);
        flat_status = Some((works.len(), metadata));
        let notice = empty_notice(true, !query.is_empty(), works.is_empty(), !franchise.is_empty()).is_some();
        flat_rows(works, cols, notice)
    } else if !query.is_empty() {
        let found = found_sections(v, &query);
        let notice = empty_notice(false, true, found.is_empty(), !franchise.is_empty()).is_some();
        search_rows(&found, cols, notice)
    } else {
        let bodies = (0..=upto)
            .map(|ix| {
                let l = LETTERS[ix];
                let body = match (v.state.catalog_letter(media, l), v.state.catalog_letter_error(media, l)) {
                    (Some(e), _) => Body::Entries(e),
                    (None, Some(e)) => Body::Failed(e),
                    (None, None) => Body::Loading,
                };
                (ix, body)
            })
            .collect();
        letter_rows(bodies, cols, upto)
    };
    let heights: Arc<[f32]> = rows.iter().map(|r| m.height(r)).collect();
    // Reabre onde estava (uma vez): a letra da rota ou a posição salva.
    let offset = cx.scroll_offset(LIST);
    let c = v.pages.catalog(&v.state, media);
    if !std::mem::replace(&mut c.restored, true) {
        let place = v.state.catalog_choice(media).place;
        let c = v.pages.catalog(&v.state, media);
        match reopen_at(c.route_letter, &place, !flat_mode && query.is_empty()) {
            Reopen::Letter(l) => {
                c.upto = c.upto.max(l);
                v.state.want_catalog_letter(media, LETTERS[l]);
                if let Some(i) = letter_row(&rows, l) {
                    cx.scroll_to(LIST, higanbana_gpui::components::item_offset(&heights, 0.0, i));
                }
            }
            Reopen::Offset(y) => cx.scroll_to(LIST, y),
            Reopen::Top => {
                v.state.want_catalog_letter(media, LETTERS[0]);
            }
        }
    } else {
        // Guarda onde está (ao voltar de uma obra).
        let top = (!flat_mode && query.is_empty()).then(|| letter_at(&rows, &heights, offset)).flatten();
        v.state.save_catalog_place(media, CatalogPlace { upto, top, offset });
    }
    // Letra com falha: redesenha depois da espera para o aviso visível pedir de novo.
    if rows.iter().any(|r| matches!(r, Row::Failed { .. })) {
        cx.request_redraw_in(RETRY_CHECK);
    }
    if let Some(until) = v.state.cloud_paused_until() {
        cx.request_redraw_in(until.saturating_duration_since(Instant::now()));
    }
    let anilist_left = v.state.anilist_paused_until().map(|u| u.saturating_duration_since(Instant::now()));
    if let Some(left) = anilist_left {
        cx.request_redraw_in(left.min(Duration::from_secs(1)));
    }
    let notice = if flat_mode { Some(EmptyNotice::NoFilterMatch) } else { Some(EmptyNotice::NoMatch) };
    let present: Vec<usize> = rows.iter().filter_map(|r| if let Row::LetterHead { letter, .. } = r { Some(*letter) } else { None }).collect();
    let current = (!flat_mode).then(|| letter_at(&rows, &heights, offset)).flatten();
    let ctx = render_row::Ctx { m, width, flat_status, anilist_left, franchise, notice };
    let rows_arc: Arc<[Row]> = rows.into();
    let list = {
        let rows = rows_arc.clone();
        VirtualList::new(LIST, heights.clone()).render(cx, |i, cx| render_row::row(v, cx, &rows[i], &ctx))
    };
    let rail = rail::rail(cx, &rows_arc, &heights, flat_mode, !query.is_empty(), &present, current);
    let t = cx.theme();
    div().size_full().bg(t.background).child(div().flex_1().min_w(0.0).h_full().child(list)).child(rail)
}
