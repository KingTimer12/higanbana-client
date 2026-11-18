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
    franchise::{FranchiseTab, franchise_span, franchise_tabs},
    library::{WorkName, base_name, folder_title, join_path, natural_cmp, parse_work_name, raw_path},
    nuvem::FileEntry,
};

use crate::{
    components::{PosterProps, breadcrumbs, columns, content_width, cover, poster, want_lookup, when_visible},
    interact::Button,
    state::{AppState, state},
};

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
        let names: Vec<String> = works.iter().map(|w| w.name.clone()).collect();
        let works = works.into_iter().map(|w| (w.name.clone(), parse_work_name(&w.name))).collect();
        let franchise = base_name(&title_path).to_owned();
        state.update(cx, |s, cx| {
            s.want_lookup(&franchise, MediaType::Anime, cx);
            // Todas as obras de uma vez (não só as visíveis): o formato do AniList/MAL decide
            // a aba, então as abas já nascem certas. `want_lookup` junta tudo em poucos lotes.
            for name in &names {
                s.want_lookup(name, MediaType::Anime, cx);
            }
            s.set_inner_works(&title_path, names);
        });
        let subs = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        Self { state, title_path, works, tab: None, _subs: subs }
    }
}

impl Render for FranchiseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, cx, 72. * rem);
        let gap = 1.25 * rem;
        let cols = columns(width, 9.5 * rem, gap);
        let card_w = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());

        let st = self.state.read(cx);
        let franchise = base_name(&self.title_path).to_owned();
        let hero_media = st.lookup(&franchise, MediaType::Anime).flatten();
        // `None` = AniList/MAL ainda não respondeu para essa obra.
        let looked: Vec<Option<Option<std::sync::Arc<MediaSummary>>>> = self.works.iter().map(|(name, _)| st.lookup(name, MediaType::Anime)).collect();
        let works: Vec<(String, WorkName, Option<std::sync::Arc<MediaSummary>>)> = self
            .works
            .iter()
            .zip(&looked)
            .map(|((name, parsed), m)| (name.clone(), parsed.clone(), m.clone().flatten()))
            .collect();
        let input: Vec<(&WorkName, Option<&str>)> = works.iter().map(|w| (&w.1, w.2.as_ref().and_then(|m| m.format.as_deref()))).collect();
        let tabs = franchise_tabs(&input);
        let active_ix = self.tab.and_then(|t| tabs.iter().position(|(tab, _)| *tab == t)).unwrap_or(0);

        let span = franchise_span(&works.iter().map(|w| &w.1).collect::<Vec<_>>());

        let raw = raw_path(&self.title_path);
        let st_entity = self.state.clone();
        let hero = h_flex()
            .items_end()
            .gap_6()
            .child(div().flex_none().shadow_lg().rounded(theme.radius_lg).child(cover(hero_media.as_deref(), &franchise, Some(px(9. * rem)), cx)))
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
                    name: name.clone().into(),
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
