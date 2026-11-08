//! Barra de filtros do catálogo de Animes: botões que abrem menus (Formato,
//! Gênero, Ano, Temporada), Dublado, chips removíveis, ordenação e coleções.
//!
//! A barra guarda os filtros escolhidos; o texto da busca continua na tela do
//! catálogo, que junta os dois para montar a lista.

use std::collections::HashSet;

use gpui_kit::{
    Anchor, AnyElement, App, Context, ElementId, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, assets::IconName, div, prelude::*, px,
};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, h_flex,
    button::{Button as KitButton, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    popover::Popover,
};
use higanbana_core::{
    anilist::{MediaType, types::MediaSummary},
    anime_index::{IndexEntry, LETTERS},
    catalog_filter::{Filters, Season, Sort, Verdict, judge, sort_key_score, sort_key_year},
    library::{WorkKind, is_dated, join_path, split_path},
    nuvem::{SearchKind, SearchResult},
};

use crate::{
    interact::{Button, Pointer},
    state::AppState,
};

/// Limites aceitos nos campos de ano.
const YEAR_MIN: i32 = 1900;
const YEAR_MAX: i32 = 2100;

const KINDS: [WorkKind; 5] = [WorkKind::Tv, WorkKind::Movie, WorkKind::Ova, WorkKind::Ona, WorkKind::Special];
const SEASONS: [Season; 4] = [Season::Winter, Season::Spring, Season::Summer, Season::Fall];
const SORTS: [Sort; 3] = [Sort::Az, Sort::Year, Sort::Score];

fn kind_label(k: WorkKind) -> &'static str {
    match k {
        WorkKind::Tv => "TV",
        WorkKind::Movie => "Filme",
        WorkKind::Ova => "OVA",
        WorkKind::Ona => "ONA",
        WorkKind::Special => "Especial",
    }
}

fn season_label(s: Season) -> &'static str {
    match s {
        Season::Winter => "Inverno",
        Season::Spring => "Primavera",
        Season::Summer => "Verão",
        Season::Fall => "Outono",
    }
}

fn sort_label(s: Sort) -> &'static str {
    match s {
        Sort::Az => "A–Z",
        Sort::Year => "Ano",
        Sort::Score => "Nota",
    }
}

/// Ano de quatro dígitos dentro dos limites; o resto não conta.
fn parse_year(text: &str) -> Option<i32> {
    let t = text.trim();
    if t.len() != 4 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok().filter(|y| (YEAR_MIN..=YEAR_MAX).contains(y))
}

/// Faixa de anos dos campos "de"/"até": invertida vira na ordem certa; um lado vazio fica aberto.
fn year_range(from: Option<i32>, to: Option<i32>) -> Option<(i32, i32)> {
    match (from, to) {
        (None, None) => None,
        (from, to) => {
            let (a, b) = (from.unwrap_or(YEAR_MIN), to.unwrap_or(YEAR_MAX));
            Some((a.min(b), a.max(b)))
        }
    }
}

/// Texto do chip de ano.
fn year_label((from, to): (i32, i32)) -> String {
    match (from, to) {
        _ if from == to => from.to_string(),
        (YEAR_MIN, to) => format!("Até {to}"),
        (from, YEAR_MAX) => format!("Desde {from}"),
        (from, to) => format!("{from}–{to}"),
    }
}

/// Gêneros dos metadados já carregados, sem repetir (sem diferenciar caixa), em ordem alfabética.
fn loaded_genres<'a>(metas: impl IntoIterator<Item = &'a MediaSummary>) -> Vec<String> {
    genres_to_offer(&[], &metas.into_iter().flat_map(|m| m.genres.iter().cloned()).collect::<Vec<_>>())
}

/// Os gêneros carregados mais os escolhidos (que sempre entram), em ordem alfabética.
fn genres_to_offer(loaded: &[String], selected: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut genres: Vec<String> = loaded.iter().chain(selected).filter(|g| seen.insert(g.to_lowercase())).cloned().collect();
    genres.sort_by_cached_key(|g| g.to_lowercase());
    genres
}

/// Liga ou desliga `x` na lista.
fn toggle<T: PartialEq>(list: &mut Vec<T>, x: T) {
    match list.iter().position(|y| *y == x) {
        Some(pos) => {
            list.remove(pos);
        }
        None => list.push(x),
    }
}

/// "Gênero · 2" quando há escolhas.
fn trigger_label(name: &str, count: usize) -> String {
    if count == 0 { name.to_owned() } else { format!("{name} · {count}") }
}

/// Número com separador de milhar: 4031 → "4.031".
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// "N obras · metadados X / Y · índice X / 27": metadados e índice só enquanto incompletos.
pub fn status_line(works: usize, (resolved, total): (usize, usize), (indexed, letters): (usize, usize)) -> String {
    let mut line = format!("{} {}", thousands(works), if works == 1 { "obra" } else { "obras" });
    if resolved < total {
        line += &format!(" · metadados {} / {}", thousands(resolved), thousands(total));
    }
    if indexed < letters {
        line += &format!(" · índice {indexed} / {letters}");
    }
    line
}

/// Aviso de lista vazia.
#[derive(Debug, PartialEq, Eq)]
pub enum EmptyNotice {
    /// "Nenhuma obra com “…”" (busca por letra).
    NoMatch,
    /// "Nenhuma obra com esses filtros" (grade única).
    NoFilterMatch,
}

/// Qual aviso de lista vazia mostrar; nenhum quando a grade tem obras ou quando
/// a busca na nuvem achou obras dentro de franquias (o aviso as contradiria).
pub fn empty_notice(flat: bool, searching: bool, nothing_shown: bool, franchise_found: bool) -> Option<EmptyNotice> {
    match (nothing_shown && !franchise_found, flat, searching) {
        (false, _, _) => None,
        (true, true, _) => Some(EmptyNotice::NoFilterMatch),
        (true, false, true) => Some(EmptyNotice::NoMatch),
        (true, false, false) => None,
    }
}

/// Texto dos campos "de"/"até" para uma faixa guardada (lado aberto fica vazio).
fn year_fields_text(years: Option<(i32, i32)>) -> (String, String) {
    let Some((from, to)) = years else { return (String::new(), String::new()) };
    let side = |y: i32, open: i32| if y == open { String::new() } else { y.to_string() };
    (side(from, YEAR_MIN), side(to, YEAR_MAX))
}

/// Uma obra do índice com o que se sabe dela.
pub struct Work<'a> {
    pub entry: &'a IndexEntry,
    pub meta: Option<&'a MediaSummary>,
    /// Obras de dentro da franquia, quando ela já foi aberta.
    pub inner: Vec<MediaSummary>,
}

/// Obras que passam nos filtros (as sem resposta ainda ficam de fora), na ordem pedida.
/// A–Z mantém a ordem do índice; Ano e Nota vão do maior ao menor, e o empate fica em A–Z.
pub fn filtered_works(works: &[Work], f: &Filters, sort: Sort) -> Vec<IndexEntry> {
    let mut found: Vec<&Work> = works.iter().filter(|w| judge(&w.entry.name, w.meta, &w.inner, f) == Verdict::Pass).collect();
    // A franquia vale pela melhor das obras conhecidas (a própria pasta ou as de dentro).
    let best = |w: &Work, key: fn(Option<&MediaSummary>) -> i64| w.meta.into_iter().chain(&w.inner).map(|m| key(Some(m))).max().unwrap_or(key(None));
    match sort {
        Sort::Az => {}
        Sort::Year => found.sort_by_key(|w| std::cmp::Reverse(best(w, |m| sort_key_year(m) as i64))),
        Sort::Score => found.sort_by_key(|w| std::cmp::Reverse(best(w, sort_key_score))),
    }
    found.into_iter().map(|w| w.entry.clone()).collect()
}

/// Resultados da busca na nuvem que são obras dentro de franquias (`Animes/Letra/Franquia/[Ano] Obra`).
/// Pastas mais fundas (versões, extras, episódios) e subpastas sem ano ficam de fora.
pub fn franchise_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|r| {
            let parts = split_path(&r.path);
            r.kind == SearchKind::Anime && parts.len() == 4 && is_dated(parts[3])
        })
        .collect()
}

/// Menus da barra que abrem num popover.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Kind,
    Genre,
    Year,
    Season,
}

/// Os filtros ou a ordenação mudaram.
pub struct FiltersChanged;

pub struct FilterBar {
    state: Entity<AppState>,
    /// Sem `query`: a busca fica na tela do catálogo.
    filters: Filters,
    sort: Sort,
    year_from: Entity<InputState>,
    year_to: Entity<InputState>,
    menu: Option<Menu>,
    /// Gêneros carregados em cache, pela contagem de metadados (`metadata_progress`).
    genres: Option<((usize, usize), Vec<String>)>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<FiltersChanged> for FilterBar {}

impl FilterBar {
    /// Começa com o que estava escolhido da última vez (guardado em `AppState`).
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = state.read(cx).catalog_choice().clone();
        let filters = Filters { query: String::new(), ..saved.filters };
        let (from, to) = year_fields_text(filters.years);
        let year_from = cx.new(|cx| InputState::new(window, cx).placeholder("De").default_value(from));
        let year_to = cx.new(|cx| InputState::new(window, cx).placeholder("Até").default_value(to));
        let on_year = |this: &mut Self, _: Entity<InputState>, e: &InputEvent, cx: &mut Context<Self>| {
            if matches!(e, InputEvent::Change) {
                this.read_years(cx);
            }
        };
        let subs = vec![cx.subscribe(&year_from, on_year), cx.subscribe(&year_to, on_year)];
        Self { state, filters, sort: saved.sort, year_from, year_to, menu: None, genres: None, _subs: subs }
    }

    pub fn filters(&self) -> &Filters {
        &self.filters
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    /// Algum filtro escolhido (a ordenação não conta).
    pub fn is_active(&self) -> bool {
        !self.filters.is_empty()
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let (filters, sort) = (self.filters.clone(), self.sort);
        self.state.update(cx, |s, _| s.save_catalog_filters(&filters, sort));
        cx.emit(FiltersChanged);
        cx.notify();
    }

    fn read_years(&mut self, cx: &mut Context<Self>) {
        let from = parse_year(&self.year_from.read(cx).value());
        let to = parse_year(&self.year_to.read(cx).value());
        let years = year_range(from, to);
        if years != self.filters.years {
            self.filters.years = years;
            self.changed(cx);
        }
    }

    fn set_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        if sort != self.sort {
            self.sort = sort;
            self.changed(cx);
        }
    }

    fn clear_years(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // `set_value` não avisa a mudança: os anos saem do filtro aqui.
        self.year_from.update(cx, |s, cx| s.set_value("", window, cx));
        self.year_to.update(cx, |s, cx| s.set_value("", window, cx));
        self.filters.years = None;
    }

    /// Tira todos os filtros; a ordenação fica.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_years(window, cx);
        self.filters = Filters::default();
        self.changed(cx);
    }

    /// Gêneros dos metadados de todas as letras já indexadas; refeito só quando chegam metadados.
    fn refresh_genres(&mut self, cx: &App) {
        let st = self.state.read(cx);
        let progress = st.metadata_progress();
        if self.genres.as_ref().is_some_and(|(k, _)| *k == progress) {
            return;
        }
        let letters: Vec<_> = LETTERS.iter().filter_map(|l| st.anime_letter(l)).collect();
        let metas = letters.iter().flat_map(|l| l.iter()).filter_map(|e| st.lookup_ref(&e.name, MediaType::Anime).flatten());
        self.genres = Some((progress, loaded_genres(metas)));
    }

    fn offered_genres(&self) -> Vec<String> {
        genres_to_offer(self.genres.as_ref().map_or(&[][..], |(_, g)| g), &self.filters.genres)
    }

    /// Botão que abre um menu da barra; o conteúdo só é montado com o menu aberto.
    fn menu_button(&self, menu: Menu, name: &'static str, count: usize, content: impl FnOnce(&Self, &mut Context<Self>) -> AnyElement, cx: &mut Context<Self>) -> Popover {
        let open = self.menu == Some(menu);
        let body = if open { content(self, cx) } else { div().into_any_element() };
        Popover::new(ElementId::Name(format!("catalog-filter-menu:{name}").into()))
            .open(open)
            .on_open_change(cx.listener(move |b, open: &bool, _, cx| {
                b.menu = if *open { Some(menu) } else if b.menu == Some(menu) { None } else { b.menu };
                cx.notify();
            }))
            .trigger(
                Button::new(ElementId::Name(format!("catalog-filter:{name}").into()))
                    .outline()
                    .small()
                    .label(trigger_label(name, count))
                    .dropdown_caret(true)
                    .selected(count > 0),
            )
            .child(body)
    }

    /// Opções em chips que ligam e desligam sem fechar o menu.
    fn options<T: Clone + PartialEq + 'static>(
        &self,
        id: &'static str,
        items: impl IntoIterator<Item = (T, String)>,
        chosen: impl Fn(&Filters) -> &Vec<T>,
        pick: fn(&mut Filters) -> &mut Vec<T>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .flex_wrap()
            .gap_1p5()
            .max_w(px(360.))
            .children(items.into_iter().map(|(x, label)| {
                let on = chosen(&self.filters).contains(&x);
                Button::new(ElementId::Name(format!("{id}:{label}").into()))
                    .outline()
                    .small()
                    .rounded(px(999.))
                    .label(label)
                    .selected(on)
                    .on_click(cx.listener(move |b, _, _, cx| {
                        toggle(pick(&mut b.filters), x.clone());
                        b.changed(cx);
                    }))
            }))
            .into_any_element()
    }

    fn year_fields(&self) -> AnyElement {
        h_flex()
            .items_center()
            .gap_2()
            .child(div().w(px(88.)).child(Input::new(&self.year_from).small()))
            .child("–")
            .child(div().w(px(88.)).child(Input::new(&self.year_to).small()))
            .into_any_element()
    }

    /// Chip de um filtro ativo; clicar tira o filtro.
    fn chip(&self, id: String, label: String, remove: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static, cx: &mut Context<Self>) -> Button {
        Button::new(ElementId::Name(id.into()))
            .outline()
            .small()
            .rounded(px(999.))
            .selected(true)
            .label(label.clone())
            .child(Icon::new(IconName::Close).xsmall())
            .tooltip(format!("Remover {label}"))
            .on_click(cx.listener(move |b, _, window, cx| {
                remove(b, window, cx);
                b.changed(cx);
            }))
    }

    fn chips(&self, cx: &mut Context<Self>) -> Vec<Button> {
        let f = self.filters.clone();
        let mut chips = Vec::new();
        for k in f.kinds {
            chips.push(self.chip(format!("chip-kind:{}", kind_label(k)), kind_label(k).into(), move |b, _, _| b.filters.kinds.retain(|x| *x != k), cx));
        }
        for g in f.genres {
            let name = g.clone();
            chips.push(self.chip(format!("chip-genre:{g}"), g, move |b, _, _| b.filters.genres.retain(|x| *x != name), cx));
        }
        if let Some(years) = f.years {
            chips.push(self.chip("chip-years".into(), year_label(years), |b, window, cx| b.clear_years(window, cx), cx));
        }
        for s in f.seasons {
            chips.push(self.chip(format!("chip-season:{}", season_label(s)), season_label(s).into(), move |b, _, _| b.filters.seasons.retain(|x| *x != s), cx));
        }
        if f.dubbed {
            chips.push(self.chip("chip-dubbed".into(), "Dublado".into(), |b, _, _| b.filters.dubbed = false, cx));
        }
        chips
    }
}

impl Render for FilterBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if self.menu == Some(Menu::Genre) {
            self.refresh_genres(cx);
        }
        let f = &self.filters;
        let (kinds, genres, seasons, years) = (f.kinds.len(), f.genres.len(), f.seasons.len(), usize::from(f.years.is_some()));
        let dubbed = f.dubbed;
        let active = self.is_active();
        let kind_menu = self.menu_button(
            Menu::Kind,
            "Formato",
            kinds,
            |b, cx| b.options("kind", KINDS.map(|k| (k, kind_label(k).to_owned())), |f| &f.kinds, |f| &mut f.kinds, cx),
            cx,
        );
        let genre_menu = self.menu_button(
            Menu::Genre,
            "Gênero",
            genres,
            |b, cx| {
                let offered = b.offered_genres();
                if offered.is_empty() {
                    return div()
                        .max_w(px(280.))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Os gêneros aparecem conforme os metadados das obras chegam.")
                        .into_any_element();
                }
                b.options("genre", offered.into_iter().map(|g| (g.clone(), g)).collect::<Vec<_>>(), |f| &f.genres, |f| &mut f.genres, cx)
            },
            cx,
        );
        let year_menu = self.menu_button(Menu::Year, "Ano", years, |b, _| b.year_fields(), cx);
        let season_menu = self.menu_button(
            Menu::Season,
            "Temporada",
            seasons,
            |b, cx| b.options("season", SEASONS.map(|s| (s, season_label(s).to_owned())), |f| &f.seasons, |f| &mut f.seasons, cx),
            cx,
        );
        let chips = if active { self.chips(cx) } else { Vec::new() };
        h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(kind_menu)
            .child(genre_menu)
            .child(year_menu)
            .child(season_menu)
            .child(
                Button::new("catalog-filter:dubbed")
                    .outline()
                    .small()
                    .label("Dublado")
                    .selected(dubbed)
                    .on_click(cx.listener(|b, _, _, cx| {
                        b.filters.dubbed = !b.filters.dubbed;
                        b.changed(cx);
                    })),
            )
            .when(active, |d| {
                d.child(div().w_px().h_5().mx_1().bg(theme.border))
                    .children(chips)
                    .child(Button::new("catalog-filter:clear").ghost().small().label("Limpar").on_click(cx.listener(|b, _, window, cx| b.clear(window, cx))))
            })
    }
}

/// "Ordenar: A–Z" com o menu das ordens.
pub fn sort_menu(bar: &Entity<FilterBar>, cx: &App) -> impl IntoElement {
    let sort = bar.read(cx).sort;
    let bar = bar.clone();
    let button = KitButton::new("catalog-sort").outline().label(format!("Ordenar: {}", sort_label(sort))).dropdown_caret(true);
    Pointer::new(
        button.dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, cx| {
            let current = bar.read(cx).sort;
            SORTS.into_iter().fold(menu, |menu, s| {
                let bar = bar.clone();
                menu.item(PopupMenuItem::new(sort_label(s)).checked(s == current).on_click(move |_, _, cx| bar.update(cx, |b, cx| b.set_sort(s, cx))))
            })
        }),
        true,
    )
}

/// Menu das coleções de `root` (pastas fora das letras); cada uma abre como estante.
pub fn collections_menu(state: &Entity<AppState>, root: &'static str, cx: &App) -> impl IntoElement {
    let empty = state.read(cx).anime_collections().is_empty();
    let st = state.clone();
    let button = KitButton::new("catalog-collections").outline().label("Coleções").dropdown_caret(true).disabled(empty);
    Pointer::new(
        button.dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, cx| {
            st.read(cx).anime_collections().into_iter().fold(menu.max_h(px(420.)).scrollable(true), |menu, name| {
                let st = st.clone();
                let target = join_path(root, &name);
                menu.item(PopupMenuItem::new(name).on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx))))
            })
        }),
        !empty,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn summary(v: serde_json::Value) -> MediaSummary {
        serde_json::from_value(v).unwrap()
    }

    fn entry(name: &str) -> IndexEntry {
        IndexEntry { letter: "Letra X".into(), name: name.into(), path: format!("Animes/Letra X/{name}") }
    }

    #[test]
    fn labels() {
        assert_eq!(KINDS.map(kind_label), ["TV", "Filme", "OVA", "ONA", "Especial"]);
        assert_eq!(SEASONS.map(season_label), ["Inverno", "Primavera", "Verão", "Outono"]);
        assert_eq!(SORTS.map(sort_label), ["A–Z", "Ano", "Nota"]);
        assert_eq!(trigger_label("Gênero", 0), "Gênero");
        assert_eq!(trigger_label("Gênero", 1), "Gênero · 1");
    }

    #[test]
    fn years() {
        assert_eq!(parse_year(" 2010 "), Some(2010));
        assert_eq!(parse_year("201"), None);
        assert_eq!(parse_year("abcd"), None);
        assert_eq!(parse_year("1500"), None);
        assert_eq!(year_range(None, None), None);
        assert_eq!(year_range(Some(2009), Some(2011)), Some((2009, 2011)));
        // Invertida: troca.
        assert_eq!(year_range(Some(2011), Some(2009)), Some((2009, 2011)));
        assert_eq!(year_range(Some(2009), None), Some((2009, YEAR_MAX)));
        assert_eq!(year_range(None, Some(2011)), Some((YEAR_MIN, 2011)));
        assert_eq!(year_label((2009, 2011)), "2009–2011");
        assert_eq!(year_label((2010, 2010)), "2010");
        assert_eq!(year_label((2009, YEAR_MAX)), "Desde 2009");
        assert_eq!(year_label((YEAR_MIN, 2011)), "Até 2011");
    }

    #[test]
    fn offers_loaded_genres() {
        let a = summary(json!({"id":1,"genres":["Mahou Shoujo","Comedy"]}));
        let b = summary(json!({"id":2,"genres":["action","Comedy"]}));
        let loaded = loaded_genres([&a, &b]);
        assert_eq!(loaded, vec!["action", "Comedy", "Mahou Shoujo"]);
        assert_eq!(genres_to_offer(&loaded, &[]), loaded);
        // O escolhido continua no menu mesmo sem obra carregada com ele; repetido em outra caixa não duplica.
        assert_eq!(genres_to_offer(&loaded, &["Sports".to_owned(), "comedy".to_owned()]), vec!["action", "Comedy", "Mahou Shoujo", "Sports"]);
        assert!(genres_to_offer(&[], &[]).is_empty());
    }

    #[test]
    fn toggles() {
        let mut v = vec![1, 2];
        toggle(&mut v, 3);
        toggle(&mut v, 1);
        assert_eq!(v, vec![2, 3]);
    }

    #[test]
    fn status() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(4031), "4.031");
        assert_eq!(thousands(1234567), "1.234.567");
        assert_eq!(status_line(74, (3120, 4031), (27, 27)), "74 obras · metadados 3.120 / 4.031");
        assert_eq!(status_line(1, (10, 10), (27, 27)), "1 obra");
        assert_eq!(status_line(0, (0, 0), (27, 27)), "0 obras");
        // Enquanto indexa, o índice aparece para o filtro não esconder obras em silêncio.
        assert_eq!(status_line(5, (10, 10), (3, 27)), "5 obras · índice 3 / 27");
        assert_eq!(status_line(5, (8, 10), (3, 27)), "5 obras · metadados 8 / 10 · índice 3 / 27");
    }

    #[test]
    fn filters_and_sorts() {
        let (a, b, c, d) = (entry("Aria"), entry("Bleach [DUAL]"), entry("Clannad"), entry("Dragon Ball"));
        let tv2010 = summary(json!({"id":1,"format":"TV","seasonYear":2010,"averageScore":70,"genres":["Comedy"]}));
        let movie2015 = summary(json!({"id":2,"format":"MOVIE","seasonYear":2015,"averageScore":85,"genres":["Action"]}));
        let tv2005 = summary(json!({"id":3,"format":"TV","seasonYear":2005,"genres":["Drama"]}));
        let works = vec![
            Work { entry: &a, meta: Some(&tv2010), inner: vec![] },
            Work { entry: &b, meta: Some(&movie2015), inner: vec![] },
            // Sem metadados ainda.
            Work { entry: &c, meta: None, inner: vec![] },
            // Franquia sem metadados próprios, com uma obra de dentro conhecida.
            Work { entry: &d, meta: None, inner: vec![tv2005.clone()] },
        ];
        let names = |v: Vec<IndexEntry>| v.into_iter().map(|e| e.name).collect::<Vec<_>>();

        // Sem filtro: todas, na ordem do índice.
        assert_eq!(names(filtered_works(&works, &Filters::default(), Sort::Az)), vec!["Aria", "Bleach [DUAL]", "Clannad", "Dragon Ball"]);
        // TV: a sem metadados some; a franquia entra pela obra de dentro.
        let tv = Filters { kinds: vec![WorkKind::Tv], ..Default::default() };
        assert_eq!(names(filtered_works(&works, &tv, Sort::Az)), vec!["Aria", "Dragon Ball"]);
        // Dublado não depende de metadados.
        let dub = Filters { dubbed: true, ..Default::default() };
        assert_eq!(names(filtered_works(&works, &dub, Sort::Az)), vec!["Bleach [DUAL]"]);
        // Ano: mais novo primeiro (a franquia vale pela obra de dentro); sem ano no fim.
        assert_eq!(names(filtered_works(&works, &Filters::default(), Sort::Year)), vec!["Bleach [DUAL]", "Aria", "Dragon Ball", "Clannad"]);
        // Nota: maior primeiro; empate (sem nota) fica em A–Z.
        assert_eq!(names(filtered_works(&works, &Filters::default(), Sort::Score)), vec!["Bleach [DUAL]", "Aria", "Clannad", "Dragon Ball"]);
        // Busca por nome junto com os filtros.
        let q = Filters { query: "clan".into(), ..Default::default() };
        assert_eq!(names(filtered_works(&works, &q, Sort::Year)), vec!["Clannad"]);
    }

    #[test]
    fn empty_state_never_contradicts_franchise_results() {
        // Algo na grade: nada de estado vazio.
        assert_eq!(empty_notice(true, true, false, false), None);
        // Busca sem filtro que não achou nada por letra.
        assert_eq!(empty_notice(false, true, true, false), Some(EmptyNotice::NoMatch));
        // Sem busca nem filtro, a lista vazia é só o índice carregando.
        assert_eq!(empty_notice(false, false, true, false), None);
        // Grade única vazia.
        assert_eq!(empty_notice(true, false, true, false), Some(EmptyNotice::NoFilterMatch));
        // Achou dentro de franquias: o aviso some nos dois modos.
        assert_eq!(empty_notice(true, true, true, true), None);
        assert_eq!(empty_notice(false, true, true, true), None);
    }

    #[test]
    fn restores_year_fields() {
        assert_eq!(year_fields_text(None), (String::new(), String::new()));
        assert_eq!(year_fields_text(Some((2009, 2011))), ("2009".into(), "2011".into()));
        assert_eq!(year_fields_text(Some((2009, YEAR_MAX))), ("2009".into(), String::new()));
        assert_eq!(year_fields_text(Some((YEAR_MIN, 2011))), (String::new(), "2011".into()));
    }

    #[test]
    fn keeps_works_inside_franchises() {
        let r = |path: &str, kind| SearchResult { name: path.rsplit('/').next().unwrap().into(), path: path.into(), kind };
        let found = franchise_results(vec![
            r("Animes/Letra P/Precure", SearchKind::Anime),
            r("Animes/Letra P/Precure/[2010] Heartcatch Precure!", SearchKind::Anime),
            r("Mangas/Letra P/Precure/Vol 1", SearchKind::Manga),
            r("/Animes/Letra H/Hunter x Hunter/[2011] Hunter x Hunter/", SearchKind::Anime),
            // Versão, extras e episódios dentro da obra não são obras.
            r("Animes/Letra P/Precure/[2010] Heartcatch Precure!/[BD] 1080p", SearchKind::Anime),
            r("Animes/Letra P/Precure/[2010] Heartcatch Precure!/Extras", SearchKind::Anime),
            // Subpasta da franquia sem ano.
            r("Animes/Letra P/Precure/Extras", SearchKind::Anime),
        ]);
        let paths: Vec<&str> = found.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, vec!["Animes/Letra P/Precure/[2010] Heartcatch Precure!", "/Animes/Letra H/Hunter x Hunter/[2011] Hunter x Hunter/"]);
    }
}
