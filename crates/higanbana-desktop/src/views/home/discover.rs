//! Carrossel "Descobrir" da entrada, sobre o `Carousel` do kit: mais bem
//! avaliados do ano, mais populares e em alta, só com obras do acervo. Cada
//! pôster vem com a posição no ranking em número grande e sobe no hover. Cada
//! aba (e tipo) confere na primeira vez que é aberta.

use std::{collections::HashMap, sync::Arc, time::Duration};

use gpui_kit::{
    AnimationExt as _, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Window,
    assets::IconName, div, prelude::*, px, relative,
};
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    carousel::{Carousel, CarouselContent, CarouselItem, CarouselNext, CarouselPrevious, CarouselState},
    h_flex,
    skeleton::Skeleton,
    tab::{Tab, TabBar},
    v_flex,
};
use higanbana_core::{
    anilist::{MediaType, Ranking},
    catalog::ListBudget,
};

use super::{Slot, discover_task, eyebrow, leave};
use crate::{
    components::{PosterProps, poster},
    interact::hover_anim,
    motion::{self, RISE_DISTANCE, ease_out_cubic},
    state::{AppState, state},
    ui::empty_state,
};

/// Obras por aba.
const PER_TAB: usize = 12;
const TABS: [(Ranking, &str); 3] = [(Ranking::TopRated, "Mais bem avaliados"), (Ranking::Popular, "Mais populares"), (Ranking::Trending, "Em alta")];
/// Alturas em rem: título, abas e fileira de pôsteres.
const HEADER_REM: f32 = 2.5;
const TABS_REM: f32 = 2.5;
const GAP_REM: f32 = 1.;
const POSTER_REM: f32 = 9.;
/// Largura reservada ao número do ranking, que fica meio por baixo da capa.
const RANK_REM: f32 = 2.75;
/// Capa 2:3 + título em duas linhas + linha de formato + folga para subir no hover.
const ROW_REM: f32 = POSTER_REM * 1.5 + 4.5;
/// Espaço lateral para as setas do kit, que ficam fora da área dos pôsteres.
const ARROWS_REM: f32 = 3.;
const HOVER: Duration = Duration::from_millis(220);

/// Altura total do bloco (para o `cached` de quem embute).
pub fn height_rem() -> f32 {
    HEADER_REM + TABS_REM + 2. * GAP_REM + ROW_REM
}

pub struct Discover {
    state: Entity<AppState>,
    budget: Arc<ListBudget>,
    media: MediaType,
    ranking: Ranking,
    slots: HashMap<(MediaType, Ranking), Slot>,
    carousel: Entity<CarouselState>,
    /// Só confere depois do banner, que tem prioridade no orçamento de listagens.
    started: bool,
}

impl Discover {
    pub fn new(budget: Arc<ListBudget>, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Capas chegam pelo AppState.
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let carousel = cx.new(|_| CarouselState::new(0));
        Self { state, budget, media: MediaType::Anime, ranking: Ranking::TopRated, slots: HashMap::new(), carousel, started: false }
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
                    let len = slot.items.len();
                    if (v.media, v.ranking) == key {
                        v.carousel.update(cx, |c, cx| c.set_item_count(len, cx));
                    }
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
        self.slots.insert(key, Slot::new(task));
    }

    fn pick(&mut self, media: MediaType, ranking: Ranking, cx: &mut Context<Self>) {
        if (media, ranking) == (self.media, self.ranking) {
            return;
        }
        leave(&mut self.slots, &(self.media, self.ranking));
        self.media = media;
        self.ranking = ranking;
        self.started = true;
        self.load(cx);
        let len = self.slots.get(&(media, ranking)).map_or(0, |s| s.items.len());
        self.carousel.update(cx, |c, cx| {
            c.set_item_count(len, cx);
            c.set_selected_index(0, cx);
        });
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
            .child(eyebrow(IconName::Compass, "Descobrir", cx))
            .child(
                TabBar::new("discover-media")
                    .segmented()
                    .small()
                    .selected_index(if media == MediaType::Anime { 0 } else { 1 })
                    .on_click(cx.listener(|v, ix: &usize, _, cx| v.pick(if *ix == 0 { MediaType::Anime } else { MediaType::Manga }, v.ranking, cx)))
                    .child(Tab::new().label("Anime"))
                    .child(Tab::new().label("Mangá")),
            );

        let tabs = h_flex().h(px(TABS_REM * rem)).child(
            TabBar::new("discover-tabs")
                .pill()
                .selected_index(active_tab)
                .on_click(cx.listener(|v, ix: &usize, _, cx| {
                    if let Some((r, _)) = TABS.get(*ix) {
                        v.pick(v.media, *r, cx);
                    }
                }))
                .children(TABS.iter().map(|(_, label)| Tab::new().label(*label))),
        );

        let row: gpui_kit::AnyElement = if items.is_empty() && done {
            div()
                .h(px(ROW_REM * rem))
                .flex()
                .items_center()
                .justify_center()
                .child(empty_state(IconName::Inbox, "Nada desta lista no acervo", Some("Tente outra aba ou o outro tipo.".into()), cx))
                .into_any_element()
        } else if items.is_empty() {
            h_flex()
                .h(px(ROW_REM * rem))
                .items_start()
                .px(px(ARROWS_REM * rem))
                .gap_4()
                .overflow_hidden()
                .children((0..8).map(|_| div().flex_none().pl(px(RANK_REM * rem)).child(Skeleton::new().w(width).h(width * 1.5).rounded(theme.radius_lg))))
                .into_any_element()
        } else {
            let slides: Vec<_> = items
                .into_iter()
                .enumerate()
                .map(|(i, (m, path))| {
                    let id = m.summary.id;
                    let st = self.state.clone();
                    let rank_color = theme.foreground;
                    let card_id = format!("discover:{media_key}:{ranking_key}:{id}");
                    let name = m.summary.title();
                    let summary = m.summary.clone();
                    // Número do ranking atrás da capa + pôster que sobe e ganha sombra no hover.
                    let card = hover_anim(ElementId::Name(format!("discover-hover:{card_id}").into()), HOVER, move |t, _, cx| {
                        let e = ease_out_cubic(t);
                        let st = st.clone();
                        let path = path.clone();
                        h_flex()
                            .items_end()
                            .child(
                                div()
                                    .w(px(RANK_REM * rem))
                                    .mb(px(3.5 * rem))
                                    .mr(px(-0.9 * rem))
                                    .text_size(px(4.75 * rem))
                                    .line_height(relative(1.))
                                    .font_weight(FontWeight::BLACK)
                                    .text_color(rank_color.opacity(0.28 + 0.45 * e))
                                    .child((i + 1).to_string()),
                            )
                            .child(
                                div()
                                    .relative()
                                    .top(px(-6. * e))
                                    .when(e > 0.01, |d| d.shadow_xl())
                                    .rounded(cx.theme().radius_lg)
                                    .child(
                                        poster(
                                            PosterProps {
                                                id: ElementId::Name(card_id.clone().into()),
                                                name: name.clone(),
                                                media: Some(summary.clone()),
                                                loading: false,
                                                width,
                                                badge: false,
                                                dimmed: false,
                                                meta: None,
                                            },
                                            cx,
                                        )
                                        .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(path.clone(), cx))),
                                    ),
                            )
                    });
                    CarouselItem::new(ElementId::Name(format!("discover-item:{media_key}:{ranking_key}:{id}").into()), i, &self.carousel)
                        .accessibility_label(m.summary.title())
                        .w_auto()
                        .pt(px(0.5 * rem))
                        .child(card)
                        .with_animation(ElementId::Name(format!("discover-in:{media_key}:{ranking_key}:{id}").into()), motion::staggered(i), |e, t| {
                            e.opacity(t).relative().top(px(RISE_DISTANCE * (1. - t)))
                        })
                })
                .collect();
            div()
                .h(px(ROW_REM * rem))
                .px(px(ARROWS_REM * rem))
                .child(
                    Carousel::new(ElementId::Name(format!("discover-carousel:{media_key}:{ranking_key}").into()), &self.carousel)
                        .accessibility_label("Descobrir")
                        .w_full()
                        .child(CarouselContent::new(&self.carousel).h(px(ROW_REM * rem)).children(slides))
                        .child(CarouselPrevious::new(&self.carousel).small())
                        .child(CarouselNext::new(&self.carousel).small()),
                )
                .into_any_element()
        };

        v_flex().size_full().gap(px(GAP_REM * rem)).child(header).child(tabs).child(row)
    }
}
