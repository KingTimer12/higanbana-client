//! Carrossel "Descobrir" da entrada: mais bem avaliados do ano, mais populares
//! e em alta, só com obras do acervo. Cada aba (e tipo) confere na primeira vez
//! que é aberta.

use std::{collections::HashMap, sync::Arc};

use gpui_kit::{
    AnimationExt as _, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, ScrollHandle, Styled,
    Task, Window, assets::IconName, div, point, prelude::*, px,
};
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _,
    button::{Button as KitButton, ButtonGroup, ButtonVariants as _},
    h_flex,
    skeleton::Skeleton,
    tab::{Tab, TabBar},
    v_flex,
};
use higanbana_core::{
    anilist::{MediaType, Ranking, types::RankedMedia},
    catalog::ListBudget,
};

use super::discover_task;
use crate::{
    components::{PosterProps, poster},
    interact::Button,
    motion,
    state::{AppState, state},
    ui::empty_state,
};

/// Obras por aba.
const PER_TAB: usize = 12;
const TABS: [(Ranking, &str); 3] = [(Ranking::TopRated, "Mais bem avaliados"), (Ranking::Popular, "Mais populares"), (Ranking::Trending, "Em alta")];
/// Alturas em rem: título, abas e fileira de pôsteres.
const HEADER_REM: f32 = 2.25;
const TABS_REM: f32 = 2.5;
const GAP_REM: f32 = 1.;
const POSTER_REM: f32 = 9.5;
/// Capa 2:3 + título em duas linhas + linha de formato.
const ROW_REM: f32 = POSTER_REM * 1.5 + 4.;

/// Altura total do bloco (para o `cached` de quem embute).
pub fn height_rem() -> f32 {
    HEADER_REM + TABS_REM + 2. * GAP_REM + ROW_REM
}

/// Deslocamento horizontal depois de rolar uma página. O GPUI guarda o
/// deslocamento como negativo (0 = início, `-max` = fim).
pub fn page_offset(current: f32, width: f32, max: f32, forward: bool) -> f32 {
    if forward { (current - width).max(-max) } else { (current + width).min(0.) }
}

struct Slot {
    items: Vec<(RankedMedia, String)>,
    done: bool,
    _task: Task<()>,
}

pub struct Discover {
    state: Entity<AppState>,
    budget: Arc<ListBudget>,
    media: MediaType,
    ranking: Ranking,
    slots: HashMap<(MediaType, Ranking), Slot>,
    scroll: ScrollHandle,
    /// Só confere depois do banner, que tem prioridade no orçamento de listagens.
    started: bool,
}

impl Discover {
    pub fn new(budget: Arc<ListBudget>, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Capas chegam pelo AppState.
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, budget, media: MediaType::Anime, ranking: Ranking::TopRated, slots: HashMap::new(), scroll: ScrollHandle::new(), started: false }
    }

    /// Começa a conferir a aba aberta (uma vez).
    pub fn start(&mut self, cx: &mut Context<Self>) {
        if !self.started {
            self.started = true;
            self.load(cx);
            cx.notify();
        }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let key = (self.media, self.ranking);
        if self.slots.contains_key(&key) {
            return;
        }
        let task = discover_task(
            &self.state,
            self.budget.clone(),
            key.0,
            key.1,
            PER_TAB,
            cx,
            move |v: &mut Self, m, path, cx| {
                if let Some(slot) = v.slots.get_mut(&key) {
                    slot.items.push((m, path));
                    cx.notify();
                }
            },
            move |v: &mut Self, cx| {
                if let Some(slot) = v.slots.get_mut(&key) {
                    slot.done = true;
                    cx.notify();
                }
            },
        );
        self.slots.insert(key, Slot { items: Vec::new(), done: false, _task: task });
    }

    fn pick(&mut self, media: MediaType, ranking: Ranking, cx: &mut Context<Self>) {
        if (media, ranking) == (self.media, self.ranking) {
            return;
        }
        self.media = media;
        self.ranking = ranking;
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.started = true;
        self.load(cx);
        cx.notify();
    }

    fn page(&mut self, forward: bool, cx: &mut Context<Self>) {
        let offset = self.scroll.offset();
        let x = page_offset(f32::from(offset.x), f32::from(self.scroll.bounds().size.width), f32::from(self.scroll.max_offset().x), forward);
        self.scroll.set_offset(point(px(x), offset.y));
        cx.notify();
    }
}

impl Render for Discover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let (media, ranking) = (self.media, self.ranking);
        let (items, done) = self.slots.get(&(media, ranking)).map_or((Vec::new(), false), |s| (s.items.clone(), s.done));
        let width = px(POSTER_REM * rem);
        let active_tab = TABS.iter().position(|(r, _)| *r == ranking).unwrap_or(0);
        let media_key = media.as_str();
        let ranking_key = format!("{ranking:?}");

        let header = h_flex()
            .h(px(HEADER_REM * rem))
            .justify_between()
            .gap_4()
            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Descobrir"))
            .child(
                ButtonGroup::new("discover-media")
                    .outline()
                    .small()
                    .child(KitButton::new("discover-anime").label("Anime").selected(media == MediaType::Anime))
                    .child(KitButton::new("discover-manga").label("Mangá").selected(media == MediaType::Manga))
                    .on_click(cx.listener(move |v, picked: &Vec<usize>, _, cx| {
                        if let Some(&i) = picked.first() {
                            v.pick(if i == 0 { MediaType::Anime } else { MediaType::Manga }, v.ranking, cx);
                        }
                    })),
            );

        let tabs = h_flex()
            .h(px(TABS_REM * rem))
            .justify_between()
            .gap_4()
            .child(
                TabBar::new("discover-tabs")
                    .underline()
                    .selected_index(active_tab)
                    .on_click(cx.listener(|v, ix: &usize, _, cx| {
                        if let Some((r, _)) = TABS.get(*ix) {
                            v.pick(v.media, *r, cx);
                        }
                    }))
                    .children(TABS.iter().map(|(_, label)| Tab::new().label(*label))),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("discover-prev").ghost().small().icon(IconName::ChevronLeft).tooltip("Voltar").on_click(cx.listener(|v, _, _, cx| v.page(false, cx))))
                    .child(Button::new("discover-next").ghost().small().icon(IconName::ChevronRight).tooltip("Avançar").on_click(cx.listener(|v, _, _, cx| v.page(true, cx)))),
            );

        let row: gpui_kit::AnyElement = if items.is_empty() && done {
            div()
                .h(px(ROW_REM * rem))
                .flex()
                .items_center()
                .justify_center()
                .child(empty_state(IconName::Inbox, "Nada desta lista no acervo", Some("Tente outra aba ou o outro tipo.".into()), cx))
                .into_any_element()
        } else {
            let pending = if done { 0 } else { PER_TAB.saturating_sub(items.len()).min(6) };
            h_flex()
                .id(ElementId::Name(format!("discover-row:{media_key}:{ranking_key}").into()))
                .h(px(ROW_REM * rem))
                .items_start()
                .gap_5()
                .overflow_x_scroll()
                .track_scroll(&self.scroll)
                .children(items.into_iter().enumerate().map(|(i, (m, path))| {
                    let st = self.state.clone();
                    let id = m.summary.id;
                    div()
                        .flex_none()
                        .child(
                            poster(
                                PosterProps {
                                    id: ElementId::Name(format!("discover:{media_key}:{ranking_key}:{id}").into()),
                                    name: m.summary.title(),
                                    media: Some(m.summary),
                                    loading: false,
                                    width,
                                    badge: false,
                                    dimmed: false,
                                    meta: None,
                                },
                                cx,
                            )
                            .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(path.clone(), cx))),
                        )
                        .with_animation(ElementId::Name(format!("discover-in:{media_key}:{ranking_key}:{id}").into()), motion::staggered(i), |e, t| e.opacity(t))
                }))
                .children((0..pending).map(|_| div().flex_none().child(Skeleton::new().w(width).h(width * 1.5).rounded(theme.radius_lg))))
                .into_any_element()
        };

        v_flex().size_full().gap(px(GAP_REM * rem)).child(header).child(tabs).child(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_forward_moves_one_width() {
        assert_eq!(page_offset(0., 800., 2000., true), -800.);
    }

    #[test]
    fn page_forward_stops_at_end() {
        assert_eq!(page_offset(-1600., 800., 2000., true), -2000.);
    }

    #[test]
    fn page_back_stops_at_start() {
        assert_eq!(page_offset(-300., 800., 2000., false), 0.);
        assert_eq!(page_offset(-1600., 800., 2000., false), -800.);
    }
}
