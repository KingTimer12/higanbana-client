//! Catálogo de Animes: todas as obras numa grade só, de 0-9 a Z, com índice
//! lateral e busca por nome.
//!
//! A tela tem rolagem própria (não a da área de trabalho): o índice lateral
//! fica fixo ao lado e precisa do `ScrollHandle` para pular até uma letra.
//! Filhos diretos do contêiner rolável: cabeçalho, uma seção por letra e o
//! rodapé que pede a próxima letra. Com filtro ou fora de A–Z, as seções dão
//! lugar a blocos de uma grade única (barra em `catalog_filters`).

use std::{ops::Range, sync::Arc, time::Duration};

use crate::interact::Button;
use gpui_kit::{
    App, Bounds, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Pixels, Render, ScrollHandle,
    Styled, Subscription, Task, Window, assets::IconName, div, point, prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex,
    button::ButtonVariants as _,
    input::{Input, InputEvent, InputState},
};
use higanbana_core::{
    anilist::MediaType,
    anime_index::{IndexEntry, LETTERS},
    catalog_filter::{Filters, Sort},
    library::norm,
    nuvem::SearchResult,
};

use crate::{
    components::{breadcrumbs, columns, content_width, title_card, when_visible},
    state::{AppState, state},
    ui::empty_state,
    views::catalog_filters::{
        EmptyNotice, FilterBar, FiltersChanged, Work, collections_menu, empty_notice, filtered_works, franchise_results, sort_menu, status_line,
    },
};

const ROOT: &str = "Animes";
/// Pouco mais que a espera de uma letra com falha (30 s em `catalog_state`).
const RETRY_CHECK: Duration = Duration::from_secs(31);
/// Espera depois da última tecla antes de buscar na nuvem.
const CLOUD_DEBOUNCE: Duration = Duration::from_millis(400);
/// Busca na nuvem só a partir deste tamanho.
const CLOUD_MIN_CHARS: usize = 3;
/// Linhas de pôsteres por bloco na grade única (cada bloco é medido e some fora da tela).
const FLAT_BLOCK_ROWS: usize = 6;

/// Rótulo da letra no índice lateral: `0-9` → `#`, `Letra F` → `F`.
fn rail_label(letter: &str) -> &str {
    if letter == "0-9" { "#" } else { letter.trim_start_matches("Letra ") }
}

/// Rótulo da letra no cabeçalho da seção.
fn header_label(letter: &str) -> &str {
    if letter == "0-9" { "0–9" } else { letter.trim_start_matches("Letra ") }
}

/// Obras cujo nome contém `query` (sem acento nem caixa), por letra; letras sem obra somem.
fn search_sections<'a>(
    letters: impl IntoIterator<Item = (usize, &'a [IndexEntry])>,
    query: &str,
) -> Vec<(usize, Arc<Vec<IndexEntry>>)> {
    let q = norm(query);
    letters
        .into_iter()
        .map(|(ix, entries)| (ix, entries.iter().filter(|e| norm(&e.name).contains(&q)).cloned().collect::<Vec<_>>()))
        .filter(|(_, found)| !found.is_empty())
        .map(|(ix, found)| (ix, Arc::new(found)))
        .collect()
}

/// O elemento está fora da faixa `[top, bottom]` (coordenadas sem rolagem).
fn outside(bounds: Bounds<Pixels>, top: Pixels, bottom: Pixels) -> bool {
    bounds.bottom() < top || bounds.top() > bottom
}

/// Filho (do contêiner rolável) que mostra hoje a letra que estava no topo.
///
/// `old`/`new` são as letras das seções do quadro anterior e do atual; `top` é o
/// filho do topo no quadro anterior (0 = cabeçalho, seções a partir de 1).
/// `None` quando o topo era o cabeçalho ou a letra saiu da tela (busca).
fn anchor_index(old: &[usize], new: &[usize], top: usize) -> Option<usize> {
    let letter = *old.get(top.checked_sub(1)?)?;
    new.iter().position(|l| *l == letter).map(|pos| 1 + pos)
}

enum Body {
    Entries(Arc<Vec<IndexEntry>>),
    Loading,
    Failed(String),
    /// Um pedaço da grade única (com filtro ou fora de A–Z): sem cabeçalho de letra.
    Flat(Arc<Vec<IndexEntry>>, Range<usize>),
}

/// O que define a lista filtrada: igual entre dois quadros, a lista em cache vale.
#[derive(PartialEq)]
struct FlatKey {
    filters: Filters,
    sort: Sort,
    indexed: usize,
    resolved: usize,
    inner_gen: u64,
}

struct Section {
    letter: usize,
    body: Body,
}

/// O que define a altura de uma seção: igual entre dois quadros, a altura medida vale.
#[derive(Clone, PartialEq)]
struct SectionKey {
    letter: usize,
    /// 0 carregando, 1 falhou, 2 com obras, 3 bloco da grade única.
    status: u8,
    count: usize,
    cols: usize,
    card_w: i32,
    search: u64,
}

pub struct CatalogView {
    state: Entity<AppState>,
    search: Entity<InputState>,
    scroll: ScrollHandle,
    /// Última letra pedida (posição em `LETTERS`): sem busca, as seções vão de 0-9 até ela.
    upto: usize,
    /// Muda a cada texto de busca e a cada filtro; invalida medidas e posição.
    search_gen: u64,
    bar: Entity<FilterBar>,
    /// Grade única em cache.
    flat: Option<(FlatKey, Arc<Vec<IndexEntry>>)>,
    /// Texto da última busca na nuvem pedida e as obras dentro de franquias que ela achou.
    cloud_query: String,
    franchise: Vec<SearchResult>,
    /// A última busca na nuvem falhou (diferente de não achar nada).
    cloud_failed: bool,
    cloud_task: Option<Task<()>>,
    /// Resultado da busca em cache: (texto, letras indexadas, seções).
    found: Option<(String, usize, Vec<(usize, Arc<Vec<IndexEntry>>)>)>,
    /// Seções do último quadro, na ordem dos filhos (o filho 0 é o cabeçalho).
    shown: Vec<SectionKey>,
    /// Um salto acabou de ser pedido: não restaurar a posição anterior.
    jumped: bool,
    /// Redesenho agendado para tentar de novo as letras que falharam.
    retry: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl CatalogView {
    pub fn new(letter: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Busca e filtros voltam como estavam ao reabrir o catálogo (abrir uma obra e voltar).
        let saved_query = state.read(cx).catalog_choice().filters.query.clone();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Buscar no catálogo").default_value(saved_query));
        let bar = cx.new(|cx| FilterBar::new(state.clone(), window, cx));
        let subs = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |this: &mut Self, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    this.restart(cx);
                    this.cloud_search(cx);
                }
            }),
            // A barra também desenha os menus de ordenação e coleções daqui.
            cx.observe(&bar, |_, _, cx| cx.notify()),
            cx.subscribe(&bar, |this: &mut Self, _, _: &FiltersChanged, cx| this.restart(cx)),
        ];
        state.update(cx, |s, cx| {
            s.start_anime_index(cx);
            s.start_metadata_crawl(cx);
        });
        let mut view = Self {
            state,
            search,
            scroll: ScrollHandle::new(),
            upto: 0,
            search_gen: 0,
            bar,
            flat: None,
            cloud_query: String::new(),
            franchise: Vec::new(),
            cloud_failed: false,
            cloud_task: None,
            found: None,
            shown: Vec::new(),
            jumped: false,
            retry: None,
            _subs: subs,
        };
        // Sem letra a tela abre no topo (cabeçalho e busca à vista); com letra, já rolada até ela.
        match letter.and_then(|l| LETTERS.iter().position(|x| *x == l)) {
            Some(start) => view.jump(start, cx),
            None => view.state.update(cx, |s, cx| s.want_anime_letter(LETTERS[0], cx)),
        }
        view
    }

    /// Rola até a letra; sem busca, também passa a desenhar até ela e a pede.
    fn jump(&mut self, letter: usize, cx: &mut Context<Self>) {
        if self.searching(cx) {
            if let Some(pos) = self.shown.iter().position(|k| k.letter == letter) {
                self.scroll.scroll_to_top_of_item(1 + pos);
                self.jumped = true;
                cx.notify();
            }
            return;
        }
        self.upto = self.upto.max(letter);
        self.state.update(cx, |s, cx| s.want_anime_letter(LETTERS[letter], cx));
        self.scroll.scroll_to_top_of_item(1 + letter);
        self.jumped = true;
        cx.notify();
    }

    /// O fim da grade apareceu: a próxima letra entra na sequência.
    fn reach(&mut self, letter: usize, cx: &mut Context<Self>) {
        if letter <= self.upto || letter >= LETTERS.len() {
            return;
        }
        self.upto = letter;
        self.state.update(cx, |s, cx| s.want_anime_letter(LETTERS[letter], cx));
        cx.notify();
    }

    fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_owned()
    }

    fn searching(&self, cx: &App) -> bool {
        !self.query(cx).is_empty()
    }

    /// Busca ou filtros mudaram: guarda a busca, volta ao topo e esquece as medidas.
    fn restart(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        self.state.update(cx, |s, _| s.save_catalog_query(&query));
        self.search_gen += 1;
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    /// Com filtro ou fora de A–Z a tela é uma grade só, sem cabeçalhos de letra.
    fn flat_mode(&self, cx: &App) -> bool {
        let bar = self.bar.read(cx);
        bar.is_active() || bar.sort() != Sort::Az
    }

    /// Pede à nuvem as obras dentro de franquias com esse nome (depois da espera).
    fn cloud_search(&mut self, cx: &mut Context<Self>) {
        let q = self.query(cx);
        if q == self.cloud_query {
            return;
        }
        self.cloud_query = q.clone();
        self.franchise.clear();
        self.cloud_failed = false;
        self.cloud_task = None;
        if q.chars().count() < CLOUD_MIN_CHARS {
            return;
        }
        let core = self.state.read(cx).core.clone();
        self.cloud_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CLOUD_DEBOUNCE).await;
            let asked = q.clone();
            let task = core.clone().spawn(async move { core.cloud().search(&asked).await });
            let results = match task.await {
                Ok(Ok(r)) => Some(r.results),
                Ok(Err(e)) => {
                    eprintln!("busca na nuvem por {q:?}: {e}");
                    None
                }
                Err(e) => {
                    eprintln!("busca na nuvem por {q:?}: tarefa interrompida: {e}");
                    None
                }
            };
            let _ = this.update(cx, |v, cx| {
                // Resposta de um texto antigo: descarta.
                if v.cloud_query == q {
                    v.cloud_failed = results.is_none();
                    v.franchise = results.map(franchise_results).unwrap_or_default();
                    cx.notify();
                }
            });
        }));
    }

    /// Obras de todas as letras indexadas que passam nos filtros, na ordem pedida.
    fn flat_works(&mut self, filters: &Filters, sort: Sort, resolved: usize, cx: &App) -> Arc<Vec<IndexEntry>> {
        let st = self.state.read(cx);
        let key = FlatKey { filters: filters.clone(), sort, indexed: st.index_progress().0, resolved, inner_gen: st.inner_works_gen() };
        if let Some((k, works)) = &self.flat
            && *k == key
        {
            return works.clone();
        }
        let letters: Vec<Arc<Vec<IndexEntry>>> = LETTERS.iter().filter_map(|l| st.anime_letter(l)).collect();
        let works: Vec<Work> = letters
            .iter()
            .flat_map(|l| l.iter())
            .map(|e| Work { entry: e, meta: st.lookup_ref(&e.name, MediaType::Anime).flatten(), inner: st.inner_summaries(&e.path) })
            .collect();
        let found = Arc::new(filtered_works(&works, filters, sort));
        self.flat = Some((key, found.clone()));
        found
    }

    /// Seções da tela: as letras em sequência ou, com busca, as que têm resultado.
    fn sections(&mut self, query: &str, cx: &App) -> Vec<Section> {
        let st = self.state.read(cx);
        if query.is_empty() {
            return (0..=self.upto)
                .map(|ix| {
                    let letter = LETTERS[ix];
                    let body = match (st.anime_letter(letter), st.anime_letter_error(letter)) {
                        (Some(entries), _) => Body::Entries(entries),
                        (None, Some(e)) => Body::Failed(e),
                        (None, None) => Body::Loading,
                    };
                    Section { letter: ix, body }
                })
                .collect();
        }
        let indexed = st.index_progress().0;
        let fresh = self.found.as_ref().is_some_and(|(q, n, _)| q == query && *n == indexed);
        if !fresh {
            let loaded: Vec<(usize, Arc<Vec<IndexEntry>>)> =
                LETTERS.iter().enumerate().filter_map(|(ix, l)| st.anime_letter(l).map(|e| (ix, e))).collect();
            let found = search_sections(loaded.iter().map(|(ix, e)| (*ix, e.as_slice())), query);
            self.found = Some((query.to_owned(), indexed, found));
        }
        let (_, _, found) = self.found.as_ref().expect("busca em cache");
        found.iter().map(|(ix, e)| Section { letter: *ix, body: Body::Entries(e.clone()) }).collect()
    }

    /// Mantém o que está no topo da tela quando seções acima mudam de altura
    /// (letras chegando pelo índice em segundo plano).
    fn keep_position(&mut self, keys: &[SectionKey], window: &mut Window) {
        if std::mem::take(&mut self.jumped) {
            return;
        }
        let (top, off) = self.scroll.logical_scroll_top();
        let old: Vec<usize> = self.shown.iter().map(|k| k.letter).collect();
        let new: Vec<usize> = keys.iter().map(|k| k.letter).collect();
        let Some(item) = anchor_index(&old, &new, top) else { return };
        // Só reposiciona se algo acima da seção do topo mudou (entrou, saiu ou cresceu).
        if self.shown[..top - 1] == keys[..item - 1] {
            return;
        }
        let handle = self.scroll.clone();
        window.on_next_frame(move |window, _| {
            if let Some(b) = handle.bounds_for_item(item) {
                let x = handle.offset().x;
                handle.set_offset(point(x, handle.bounds().top() - b.top() + off));
                window.refresh();
            }
        });
    }

    fn render_section(&self, section: Section, width: f32, cols: usize, card_w: Pixels, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let grid = || div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8();
        if let Body::Flat(works, range) = section.body {
            let cards = works[range].iter().map(|e| {
                let id = ElementId::Name(format!("anime:{}", e.path).into());
                title_card(id, &e.name, e.path.clone(), MediaType::Anime, card_w, cx)
            });
            return div().w(px(width)).mx_auto().pt_8().child(grid().children(cards.collect::<Vec<_>>())).into_any_element();
        }
        let letter = LETTERS[section.letter];
        let count = match &section.body {
            Body::Entries(e) => Some(e.len()),
            _ => None,
        };
        let header = h_flex()
            .items_center()
            .gap_3()
            .child(
                h_flex()
                    .items_baseline()
                    .gap_2()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(header_label(letter).to_owned()))
                    .children(count.map(|n| div().text_sm().text_color(theme.muted_foreground).child(format!("· {n}")))),
            )
            .child(div().flex_1().h_px().bg(theme.border));
        let want_letter = {
            let st = self.state.clone();
            move |cx: &mut App| st.update(cx, |s, cx| s.want_anime_letter(letter, cx))
        };
        let body = match section.body {
            Body::Entries(entries) if entries.is_empty() => div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Nenhuma obra nesta letra.")
                .into_any_element(),
            Body::Entries(entries) => grid()
                .children(entries.iter().map(|e| {
                    let id = ElementId::Name(format!("anime:{}", e.path).into());
                    title_card(id, &e.name, e.path.clone(), MediaType::Anime, card_w, cx)
                }))
                .into_any_element(),
            Body::Loading => grid()
                .relative()
                .children((0..cols).map(|_| Skeleton::new().w(card_w).h(card_w * 1.5).rounded(theme.radius_lg)))
                .child(when_visible(400., want_letter))
                .into_any_element(),
            Body::Failed(e) => h_flex()
                .relative()
                .items_start()
                .gap_3()
                .p_4()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .child(Icon::new(IconName::TriangleAlert).text_color(theme.warning))
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(format!("Não foi possível carregar {}", header_label(letter))))
                        .child(div().text_sm().text_color(theme.muted_foreground).child(format!("{e}. Vamos tentar de novo em instantes."))),
                )
                .child(when_visible(400., want_letter))
                .into_any_element(),
            Body::Flat(..) => unreachable!("bloco da grade única é desenhado acima"),
        };
        v_flex().w(px(width)).mx_auto().pt_8().gap_4().child(header).child(body).into_any_element()
    }
}

impl Render for CatalogView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let rail_w = 2.5 * rem;
        let width = (content_width(window, 80. * rem + rail_w) - rail_w).max(200.);
        let gap = 1.25 * rem;
        let cols = columns(width, 10. * rem, gap);
        let card_w = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());
        let query = self.query(cx);
        let (indexed, total) = self.state.read(cx).index_progress();
        let flat_mode = self.flat_mode(cx);
        // Grade única: a lista filtrada em blocos de algumas linhas (seção = bloco).
        let mut flat_status = None;
        let sections = if flat_mode {
            let metadata = self.state.read(cx).metadata_progress();
            let (filters, sort) = {
                let bar = self.bar.read(cx);
                (Filters { query: query.clone(), ..bar.filters().clone() }, bar.sort())
            };
            let works = self.flat_works(&filters, sort, metadata.0, cx);
            flat_status = Some((works.len(), metadata));
            let block = cols * FLAT_BLOCK_ROWS;
            (0..works.len().div_ceil(block))
                .map(|ix| Section { letter: ix, body: Body::Flat(works.clone(), ix * block..((ix + 1) * block).min(works.len())) })
                .collect()
        } else {
            self.sections(&query, cx)
        };

        let keys: Vec<SectionKey> = sections
            .iter()
            .map(|s| {
                let (status, count) = match &s.body {
                    Body::Loading => (0, 0),
                    Body::Failed(_) => (1, 0),
                    Body::Entries(e) => (2, e.len()),
                    Body::Flat(_, range) => (3, range.len()),
                };
                SectionKey { letter: s.letter, status, count, cols, card_w: f32::from(card_w) as i32, search: self.search_gen }
            })
            .collect();
        // Na grade única os blocos não são letras: nada a ancorar.
        if !flat_mode {
            self.keep_position(&keys, window);
        }

        // Seções longe da tela viram um espaço da mesma altura medida no quadro anterior.
        let viewport = self.scroll.bounds();
        let offset = self.scroll.offset().y;
        let margin = viewport.size.height;
        let (top, bottom) = (viewport.top() - offset - margin, viewport.bottom() - offset + margin);
        let measured: Vec<Option<Bounds<Pixels>>> = (0..self.shown.len()).map(|pos| self.scroll.bounds_for_item(1 + pos)).collect();
        let current = if flat_mode { None } else { self.shown.get(self.scroll.top_item().saturating_sub(1)).map(|k| k.letter) };
        let present: Vec<usize> = keys.iter().map(|k| k.letter).collect();
        let last_loaded = matches!(sections.last().map(|s| &s.body), Some(Body::Entries(_)));
        // Letra com falha: redesenha depois da espera para o aviso visível pedir de novo.
        if self.retry.is_none() && sections.iter().any(|s| matches!(s.body, Body::Failed(_))) {
            self.retry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(RETRY_CHECK).await;
                let _ = this.update(cx, |v, cx| {
                    v.retry = None;
                    cx.notify();
                });
            }));
        }

        let mut children: Vec<gpui_kit::AnyElement> = Vec::with_capacity(sections.len() + 2);
        children.push(self.render_header(&query, width, (indexed, total), flat_status, (cols, card_w), cx));
        for (pos, section) in sections.into_iter().enumerate() {
            let same = self.shown.get(pos) == Some(&keys[pos]);
            match measured.get(pos).copied().flatten() {
                Some(b) if same && viewport.size.height > px(0.) && outside(b, top, bottom) => {
                    children.push(div().w(px(width)).mx_auto().h(b.size.height).into_any_element())
                }
                _ => children.push(self.render_section(section, width, cols, card_w, cx)),
            }
        }
        // Rodapé: chegar ao fim da última letra pede a próxima (só sem busca nem filtro;
        // na grade única as letras chegam pela indexação em segundo plano).
        let next = self.upto + 1;
        let view = cx.entity().downgrade();
        children.push(
            div()
                .relative()
                .h_16()
                .when(!flat_mode && query.is_empty() && last_loaded && next < LETTERS.len(), |d| {
                    d.child(when_visible(600., move |cx: &mut App| {
                        let _ = view.update(cx, |v, cx| v.reach(next, cx));
                    }))
                })
                .into_any_element(),
        );
        let notice = empty_notice(flat_mode, !query.is_empty(), present.is_empty(), !self.franchise.is_empty());
        if notice == Some(EmptyNotice::NoFilterMatch) {
            let pending = flat_status.is_some_and(|(_, (resolved, all))| resolved < all) || indexed < total;
            let detail = pending.then(|| "Ainda carregando o acervo; mais obras podem aparecer.".into());
            let clear = Button::new("catalog-clear-filters").outline().label("Limpar filtros").on_click(cx.listener(|v, _, window, cx| {
                v.bar.update(cx, |b, cx| b.clear(window, cx));
                v.search.update(cx, |s, cx| s.set_value("", window, cx));
                v.restart(cx);
                v.cloud_search(cx);
            }));
            let empty = empty_state(IconName::Search, "Nenhuma obra com esses filtros", detail, cx).child(div().pt_2().child(clear));
            children.insert(1, empty.into_any_element());
        } else if notice == Some(EmptyNotice::NoMatch) {
            let detail = (indexed < total).then(|| "Ainda indexando o acervo; mais obras podem aparecer.".into());
            children.insert(1, empty_state(IconName::Search, format!("Nenhuma obra com “{query}”"), detail, cx).into_any_element());
        }
        self.shown = keys;

        let searching = !query.is_empty();
        // Coluna flex: a margem automática do filho centraliza o índice e, se não couber, ele rola sem cortar o topo.
        let rail = v_flex().id("catalog-rail").flex_none().items_center().w(px(rail_w)).h_full().overflow_y_scroll().child(
            v_flex().my_auto().py_4().items_center().children(LETTERS.iter().enumerate().map(|(ix, l)| {
                Button::new(ElementId::Name(format!("rail:{l}").into()))
                    .ghost()
                    .xsmall()
                    .label(rail_label(l).to_owned())
                    .selected(current == Some(ix))
                    .disabled(flat_mode || (searching && !present.contains(&ix)))
                    .on_click(cx.listener(move |v, _, _, cx| v.jump(ix, cx)))
            })),
        );

        h_flex()
            .size_full()
            .items_stretch()
            .bg(theme.background)
            .child(
                div()
                    .id("catalog-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(children),
            )
            .child(rail)
    }
}

impl CatalogView {
    fn render_header(
        &self,
        query: &str,
        width: f32,
        (indexed, total): (usize, usize),
        flat_status: Option<(usize, (usize, usize))>,
        (cols, card_w): (usize, Pixels),
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let franchise = (!self.franchise.is_empty()).then(|| {
            let cards: Vec<_> = self
                .franchise
                .iter()
                .map(|r| title_card(ElementId::Name(format!("franchise:{}", r.path).into()), &r.name, r.path.clone(), MediaType::Anime, card_w, cx))
                .collect();
            v_flex()
                .pt_4()
                .gap_4()
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Dentro de franquias"))
                        .child(div().flex_1().h_px().bg(theme.border)),
                )
                .child(div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children(cards))
        });
        let status = flat_status.map(|(works, (resolved, all))| {
            let pending = resolved < all || indexed < total;
            h_flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(theme.muted_foreground)
                .when(pending, |d| d.child(div().size_1p5().rounded_full().bg(theme.primary)))
                .child(status_line(works, (resolved, all), (indexed, total)))
        });
        v_flex()
            .w(px(width))
            .mx_auto()
            .pt_6()
            .gap_4()
            .child(breadcrumbs(ROOT, cx))
            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(ROOT))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.search).prefix(Icon::new(IconName::Search).small()).cleanable(true)))
                    .child(sort_menu(&self.bar, cx))
                    .child(collections_menu(&self.state, ROOT, cx)),
            )
            .child(self.bar.clone())
            .children(status)
            .when(flat_status.is_none() && !query.is_empty() && indexed < total, |d| {
                d.child(div().text_xs().text_color(theme.muted_foreground).child(format!("Indexando nomes… {indexed}/{total}")))
            })
            .children(franchise)
            .when(self.cloud_failed, |d| d.child(div().text_xs().text_color(theme.muted_foreground).child("Busca na nuvem indisponível agora")))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(letter: &str, name: &str) -> IndexEntry {
        IndexEntry { letter: letter.into(), name: name.into(), path: format!("Animes/{letter}/{name}") }
    }

    #[test]
    fn labels() {
        assert_eq!((rail_label("0-9"), rail_label("Letra F")), ("#", "F"));
        assert_eq!((header_label("0-9"), header_label("Letra Z")), ("0–9", "Z"));
    }

    #[test]
    fn search_hides_empty_letters() {
        let f = vec![entry("Letra F", "Fresh Precure!"), entry("Letra F", "Fullmetal Alchemist")];
        let h = vec![entry("Letra H", "Heartcatch PreCure!")];
        let n = vec![entry("Letra N", "Naruto")];
        let found = search_sections([(6, f.as_slice()), (8, h.as_slice()), (14, n.as_slice())], "précure");
        let names: Vec<(usize, Vec<&str>)> =
            found.iter().map(|(ix, e)| (*ix, e.iter().map(|e| e.name.as_str()).collect())).collect();
        assert_eq!(names, vec![(6, vec!["Fresh Precure!"]), (8, vec!["Heartcatch PreCure!"])]);
    }

    #[test]
    fn anchors_by_letter() {
        // Topo no cabeçalho: nada a manter.
        assert_eq!(anchor_index(&[6, 8], &[6, 8], 0), None);
        // Sem busca, letras só crescem no fim: mesmo filho.
        assert_eq!(anchor_index(&[0, 1, 2], &[0, 1, 2, 3], 3), Some(3));
        // Busca: F (6) entrou acima de H (8), que estava no topo → H agora é o filho 3.
        assert_eq!(anchor_index(&[2, 8, 14], &[2, 6, 8, 14], 2), Some(3));
        // A letra do topo sumiu dos resultados.
        assert_eq!(anchor_index(&[2, 8], &[2, 14], 2), None);
        // Topo além das seções conhecidas (rodapé/estado vazio).
        assert_eq!(anchor_index(&[2], &[2], 5), None);
    }

    #[test]
    fn outside_range() {
        let b = Bounds::new(point(px(0.), px(100.)), gpui_kit::size(px(10.), px(50.)));
        assert!(!outside(b, px(0.), px(120.)));
        assert!(!outside(b, px(140.), px(300.)));
        assert!(outside(b, px(151.), px(300.)));
        assert!(outside(b, px(0.), px(99.)));
    }
}
