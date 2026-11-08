//! Página de uma franquia: pasta cujas subpastas são obras datadas
//! (`[2004] Futari wa Precure [TV]`, `[2005.1] Movie 1 [FILME]`).
//! Custa uma listagem só (a da franquia, feita por quem abre a página):
//! cada obra se apresenta pelo nome da pasta e pelo AniList/MAL.

use gpui_kit::{
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    assets::IconName, div, prelude::*, px, relative,
};
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, h_flex, v_flex,
    button::ButtonVariants as _,
    tab::{Tab, TabBar},
};
use higanbana_core::{
    anilist::{MediaType, types::MediaSummary},
    library::{WorkKind, WorkName, base_name, folder_title, join_path, natural_cmp, parse_work_name, raw_path},
    nuvem::FileEntry,
};

use crate::{
    components::{PosterProps, breadcrumbs, columns, content_width, cover, poster, want_lookup, when_visible},
    interact::Button,
    state::{AppState, state},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FranchiseTab {
    Tv,
    Movies,
    Specials,
    Others,
}

impl FranchiseTab {
    fn label(self, count: usize) -> String {
        match self {
            FranchiseTab::Tv => format!("Séries TV · {count}"),
            FranchiseTab::Movies => format!("Filmes · {count}"),
            FranchiseTab::Specials => format!("Especiais · {count}"),
            FranchiseTab::Others => "Outros".to_owned(),
        }
    }
}

/// Aba de uma obra. Sem ano → Outros; com ano, o formato do AniList/MAL vence
/// a tag do nome, e obra sem formato conhecido conta como série.
fn tab_of(name: &WorkName, meta: Option<&MediaSummary>) -> FranchiseTab {
    if name.year.is_none() {
        return FranchiseTab::Others;
    }
    match meta.and_then(|m| m.format.as_deref()).and_then(WorkKind::from_anilist).or(name.kind) {
        Some(WorkKind::Movie) => FranchiseTab::Movies,
        Some(WorkKind::Ova | WorkKind::Special) => FranchiseTab::Specials,
        Some(WorkKind::Tv | WorkKind::Ona) | None => FranchiseTab::Tv,
    }
}

/// Abas não vazias, na ordem Séries TV · Filmes · Especiais · Outros, com os
/// índices das obras em ordem de `(ano, ordem)` (empate mantém a ordem de entrada).
pub fn franchise_tabs(works: &[(String, WorkName, Option<MediaSummary>)]) -> Vec<(FranchiseTab, Vec<usize>)> {
    [FranchiseTab::Tv, FranchiseTab::Movies, FranchiseTab::Specials, FranchiseTab::Others]
        .into_iter()
        .filter_map(|tab| {
            let mut ix: Vec<usize> = (0..works.len()).filter(|&i| tab_of(&works[i].1, works[i].2.as_ref()) == tab).collect();
            ix.sort_by_key(|&i| (works[i].1.year, works[i].1.order));
            (!ix.is_empty()).then_some((tab, ix))
        })
        .collect()
}

pub struct FranchiseView {
    state: Entity<AppState>,
    title_path: String,
    /// Subpastas da franquia (nome completo e lido), em ordem natural.
    works: Vec<(String, WorkName)>,
    /// Aba escolhida; `None` = a primeira que existir.
    tab: Option<FranchiseTab>,
    _subs: Vec<Subscription>,
}

impl FranchiseView {
    pub fn new(title_path: String, works: Vec<FileEntry>, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let mut works: Vec<FileEntry> = works.into_iter().filter(|w| w.is_directory).collect();
        works.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        let names = works.iter().map(|w| w.name.clone()).collect();
        let works = works.into_iter().map(|w| (w.name.clone(), parse_work_name(&w.name))).collect();
        let franchise = base_name(&title_path).to_owned();
        state.update(cx, |s, cx| {
            s.set_inner_works(&title_path, names);
            s.want_lookup(&franchise, MediaType::Anime, cx);
        });
        let subs = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        Self { state, title_path, works, tab: None, _subs: subs }
    }
}

impl Render for FranchiseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, 72. * rem);
        let gap = 1.25 * rem;
        let cols = columns(width, 9.5 * rem, gap);
        let card_w = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());

        let st = self.state.read(cx);
        let franchise = base_name(&self.title_path).to_owned();
        let hero_media = st.lookup(&franchise, MediaType::Anime).flatten();
        // `None` = AniList/MAL ainda não respondeu para essa obra.
        let looked: Vec<Option<Option<MediaSummary>>> = self.works.iter().map(|(name, _)| st.lookup(name, MediaType::Anime)).collect();
        let works: Vec<(String, WorkName, Option<MediaSummary>)> = self
            .works
            .iter()
            .zip(&looked)
            .map(|((name, parsed), m)| (name.clone(), parsed.clone(), m.clone().flatten()))
            .collect();
        let tabs = franchise_tabs(&works);
        let active_ix = self.tab.and_then(|t| tabs.iter().position(|(tab, _)| *tab == t)).unwrap_or(0);

        let years: Vec<u32> = works.iter().filter_map(|w| w.1.year).collect();
        let span = match (years.iter().min(), years.iter().max()) {
            (Some(a), Some(b)) if a != b => format!("Franquia · {a}–{b}"),
            (Some(a), _) => format!("Franquia · {a}"),
            _ => "Franquia".to_owned(),
        };

        let raw = raw_path(&self.title_path);
        let st_entity = self.state.clone();
        let hero = h_flex()
            .items_end()
            .gap_6()
            .child(div().flex_none().shadow_lg().rounded(theme.radius_lg).child(cover(hero_media.as_ref(), &franchise, Some(px(9. * rem)), cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_3xl().line_height(relative(1.1)).font_weight(FontWeight::BOLD).child(folder_title(&franchise)))
                            .child(div().text_sm().text_color(theme.muted_foreground).child(span)),
                    )
                    .child(h_flex().child(Button::new("raw-folder").ghost().small().icon(IconName::Folder).label("Ver pasta").on_click(move |_, _, cx| {
                        st_entity.update(cx, |s, cx| s.go(raw.clone(), cx));
                    }))),
            );

        // Uma aba só dispensa a barra.
        let tab_bar = (tabs.len() > 1).then(|| {
            let ids: Vec<FranchiseTab> = tabs.iter().map(|t| t.0).collect();
            TabBar::new("franchise-tabs")
                .underline()
                .selected_index(active_ix)
                .on_click(cx.listener(move |v, ix: &usize, _, cx| {
                    if let Some(t) = ids.get(*ix) {
                        v.tab = Some(*t);
                        cx.notify();
                    }
                }))
                .children(tabs.iter().map(|(tab, ix)| Tab::new().cursor_pointer().label(tab.label(ix.len()))))
        });

        let shown = tabs.get(active_ix).map(|t| t.1.clone()).unwrap_or_default();
        let grid = div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children(shown.into_iter().map(|i| {
            let (name, work, meta) = &works[i];
            let pending = looked[i].is_none();
            let year = work.year.map(|y| y.to_string()).or_else(|| meta.as_ref().and_then(|m| m.year()).map(|y| y.to_string()));
            let episodes = meta.as_ref().and_then(|m| m.episodes).filter(|&e| e > 1).map(|e| format!("{e} ep"));
            let line = [year, episodes].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            let target = join_path(&self.title_path, name);
            let st = self.state.clone();
            let card = poster(
                PosterProps {
                    id: ElementId::Name(format!("work:{name}").into()),
                    name: name.clone(),
                    media: meta.clone(),
                    loading: pending,
                    width: card_w,
                    badge: false,
                    dimmed: false,
                    meta: Some(line),
                },
                cx,
            )
            .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx)));
            // Nome completo da subpasta: o casamento usa o ano.
            div().relative().child(card).when(pending, |d| d.child(when_visible(400., want_lookup(name.clone(), MediaType::Anime))))
        }));

        v_flex()
            .w(px(width))
            .mx_auto()
            .pt_6()
            .pb_16()
            .gap_8()
            .child(breadcrumbs(&self.title_path, cx))
            .child(hero)
            .child(v_flex().gap_6().children(tab_bar).child(grid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn work(name: &str, meta: Option<serde_json::Value>) -> (String, WorkName, Option<MediaSummary>) {
        (name.to_owned(), parse_work_name(name), meta.map(|v| serde_json::from_value(v).unwrap()))
    }

    #[test]
    fn franchise_tabs_by_kind() {
        let works = vec![
            work("[2005.1] B [FILME]", None),
            work("[2019.0] Star Twinkle Precure [FILME]", Some(json!({"id":1,"format":"TV"}))),
            work("Extras sem ano", None),
            work("[2004] A [TV]", None),
        ];
        let tabs = franchise_tabs(&works);
        assert_eq!(tabs, vec![(FranchiseTab::Tv, vec![3, 1]), (FranchiseTab::Movies, vec![0]), (FranchiseTab::Others, vec![2])]);
    }

    #[test]
    fn franchise_tabs_other_kinds_and_fallbacks() {
        let works = vec![
            work("[2008] Splash [OVA]", None),
            work("[2009] Gala [ESPECIAL]", None),
            work("[2010] Web [ONA]", None),
            work("[2011] Sem tag", None),
            work("[2012] Filme [FILME]", Some(json!({"id":2,"format":"FORMATO_NOVO"}))),
            work("[2013] Curta [TV]", Some(json!({"id":3,"format":"OVA"}))),
        ];
        let tabs = franchise_tabs(&works);
        assert_eq!(tabs, vec![(FranchiseTab::Tv, vec![2, 3]), (FranchiseTab::Movies, vec![4]), (FranchiseTab::Specials, vec![0, 1, 5])]);
    }
}
