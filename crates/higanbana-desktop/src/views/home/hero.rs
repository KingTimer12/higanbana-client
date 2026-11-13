//! Banner de destaques do ano na entrada: um slide largo por vez, com as obras
//! mais populares do ano que estão no acervo. Alterna Anime/Mangá; cada tipo
//! só começa a conferir quando é escolhido pela primeira vez.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::{
    AnimationExt as _, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, SpringAnimation, SpringConfig,
    Styled, Task, Window, assets::IconName, div, prelude::*, px, relative,
};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _,
    button::{Button as KitButton, ButtonGroup, ButtonVariants as _},
    h_flex,
    skeleton::Skeleton,
    tag::Tag,
    v_flex,
};
use higanbana_core::{
    anilist::{MediaType, Ranking, current_year, text, types::RankedMedia},
    catalog::ListBudget,
};

use super::discover_task;
use crate::{
    components::{cover, hex_color, remote_image},
    interact::Button,
    motion,
    state::{AppState, state},
    ui::empty_state,
};

/// Obras no banner.
const SLIDES: usize = 6;
/// Tempo de cada slide sem interação.
const AUTO_ADVANCE: Duration = Duration::from_secs(8);
/// Altura do cartão do banner e do cabeçalho, em rem.
const CARD_REM: f32 = 22.;
const HEADER_REM: f32 = 2.25;
const GAP_REM: f32 = 1.;

/// Altura total do bloco (para o `cached` de quem embute).
pub fn height_rem() -> f32 {
    HEADER_REM + GAP_REM + CARD_REM
}

/// Slide atual de um carrossel que dá a volta nas pontas.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SlideIndex {
    len: usize,
    i: usize,
}

impl SlideIndex {
    pub fn get(&self) -> usize {
        self.i
    }

    pub fn next(&mut self) {
        if self.len > 0 {
            self.i = (self.i + 1) % self.len;
        }
    }

    pub fn prev(&mut self) {
        if self.len > 0 {
            self.i = (self.i + self.len - 1) % self.len;
        }
    }

    /// Ajusta ao novo total de slides sem sair do intervalo.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.i = self.i.min(len.saturating_sub(1));
    }

    pub fn go(&mut self, i: usize) {
        if self.len > 0 {
            self.i = i.min(self.len - 1);
        }
    }
}

/// Obras confirmadas de um tipo e a conferência em andamento.
struct Slot {
    items: Vec<(RankedMedia, String)>,
    done: bool,
    _task: Task<()>,
}

pub struct Hero {
    state: Entity<AppState>,
    budget: Arc<ListBudget>,
    selected: MediaType,
    slots: HashMap<MediaType, Slot>,
    index: SlideIndex,
    hovered: bool,
    /// Última navegação manual: o auto-avanço espera um ciclo depois dela.
    touched: Instant,
    _timer: Task<()>,
}

impl Hero {
    pub fn new(budget: Arc<ListBudget>, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Imagens (banner, capa) chegam pelo AppState.
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let timer = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(AUTO_ADVANCE).await;
                let alive = this.update(cx, |h, cx| {
                    if !h.hovered && h.touched.elapsed() >= AUTO_ADVANCE && h.index.len > 1 {
                        h.index.next();
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let mut hero = Self {
            state,
            budget,
            selected: MediaType::Anime,
            slots: HashMap::new(),
            index: SlideIndex::default(),
            hovered: false,
            touched: Instant::now(),
            _timer: timer,
        };
        hero.load(MediaType::Anime, cx);
        hero
    }

    fn load(&mut self, media: MediaType, cx: &mut Context<Self>) {
        if self.slots.contains_key(&media) {
            return;
        }
        let task = discover_task(
            &self.state,
            self.budget.clone(),
            media,
            Ranking::Popular,
            SLIDES,
            cx,
            move |h: &mut Self, m, path, cx| {
                if let Some(slot) = h.slots.get_mut(&media) {
                    slot.items.push((m, path));
                    if h.selected == media {
                        h.index.set_len(slot.items.len());
                    }
                    cx.notify();
                }
            },
            move |h: &mut Self, cx| {
                if let Some(slot) = h.slots.get_mut(&media) {
                    slot.done = true;
                    cx.notify();
                }
            },
        );
        self.slots.insert(media, Slot { items: Vec::new(), done: false, _task: task });
    }

    /// O tipo mostrado já terminou de conferir (o orçamento fica livre para os outros blocos).
    pub fn settled(&self) -> bool {
        self.slots.get(&self.selected).is_some_and(|s| s.done)
    }

    fn select(&mut self, media: MediaType, cx: &mut Context<Self>) {
        if self.selected == media {
            return;
        }
        self.selected = media;
        self.load(media, cx);
        self.index = SlideIndex::default();
        self.index.set_len(self.slots.get(&media).map_or(0, |s| s.items.len()));
        self.touched = Instant::now();
        cx.notify();
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        if forward { self.index.next() } else { self.index.prev() }
        self.touched = Instant::now();
        cx.notify();
    }

    fn slide(&self, m: &RankedMedia, path: &str, rem: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let s = &m.summary;
        let title = s.title();
        let banner = s.banner_image.clone();
        let meta: Vec<String> = [
            s.format.as_deref().map(|f| text::format(Some(f))).filter(|f| !f.is_empty()),
            match self.selected {
                MediaType::Anime => s.episodes.map(|n| format!("{n} episódios")),
                MediaType::Manga => s.chapters.map(|n| format!("{n} capítulos")),
            },
            s.year().map(|y| y.to_string()),
            m.main_studio(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let chips: Vec<String> = s.genres.iter().take(4).map(|g| text::genre(g)).chain(m.top_tags(3)).collect();
        let synopsis = text::clean_description(m.description.as_deref()).0.join(" ");
        let target = path.to_owned();
        let st = self.state.clone();

        div()
            .size_full()
            .relative()
            .map(|d| match banner.as_deref() {
                Some(url) => d.child(div().absolute().inset_0().opacity(0.6).child(remote_image(Some(url), cx))),
                None => d.bg(hex_color(s.cover_image.as_ref().and_then(|c| c.color.as_deref())).unwrap_or(theme.muted).opacity(0.35)),
            })
            // Véus do tema: texto legível sobre qualquer banner, claro ou escuro.
            .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                90.,
                gpui_kit::linear_color_stop(theme.background.opacity(0.95), 0.2),
                gpui_kit::linear_color_stop(theme.background.opacity(0.), 0.9),
            )))
            .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                180.,
                gpui_kit::linear_color_stop(theme.background.opacity(0.), 0.5),
                gpui_kit::linear_color_stop(theme.background.opacity(0.7), 1.),
            )))
            .child(
                h_flex()
                    .absolute()
                    .inset_0()
                    .px_10()
                    .gap_8()
                    .child(div().flex_none().shadow_lg().rounded(theme.radius_lg).child(cover(Some(s), &title, Some(px(10.5 * rem)), cx)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .max_w(px(40. * rem))
                            .gap_3()
                            .child(div().text_3xl().line_height(relative(1.1)).font_weight(FontWeight::BOLD).line_clamp(2).child(title))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .children(s.average_score.map(|score| {
                                        h_flex()
                                            .gap_1()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child(Icon::new(IconName::Star).small().text_color(theme.yellow))
                                            .child(format!("{score}%"))
                                    }))
                                    .when(!meta.is_empty(), |d| d.child(meta.join(" · "))),
                            )
                            .when(!chips.is_empty(), |d| {
                                d.child(h_flex().flex_wrap().gap_1p5().children(chips.into_iter().map(|c| Tag::secondary().small().child(c))))
                            })
                            .when(!synopsis.is_empty(), |d| {
                                d.child(div().text_sm().line_height(relative(1.55)).text_color(theme.foreground.opacity(0.85)).line_clamp(3).child(synopsis))
                            })
                            .child(
                                h_flex().pt_1().child(
                                    Button::new("hero-open")
                                        .icon(IconName::ChevronRight)
                                        .label("Abrir")
                                        .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx))),
                                ),
                            ),
                    ),
            )
            .with_animation(
                ElementId::Name(format!("hero-slide:{}:{}", self.selected.as_str(), s.id).into()),
                motion::enter(motion::BASE),
                |e, t| e.opacity(t),
            )
            .into_any_element()
    }

    /// Pontos (o ativo cresce com mola) e setas.
    fn controls(&self, items: &[(RankedMedia, String)], cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let active = self.index.get();
        let media = self.selected.as_str();
        h_flex()
            .absolute()
            .right_6()
            .bottom_5()
            .gap_3()
            .child(h_flex().gap_1p5().children(items.iter().enumerate().map(|(i, (m, _))| {
                let on = i == active;
                div()
                    .id(ElementId::Name(format!("hero-dot:{media}:{}", m.summary.id).into()))
                    .h_1p5()
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if on { theme.foreground } else { theme.foreground.opacity(0.3) })
                    .on_click(cx.listener(move |h, _, _, cx| {
                        h.index.go(i);
                        h.touched = Instant::now();
                        cx.notify();
                    }))
                    .with_spring(
                        ElementId::Name(format!("hero-dot-w:{media}:{}", m.summary.id).into()),
                        SpringAnimation::new(SpringConfig::new(320., 30., 1.)).to(if on { 1f32 } else { 0. }),
                        |d, v| d.w(px(6. + 16. * v)),
                    )
            })))
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("hero-prev").ghost().small().icon(IconName::ChevronLeft).tooltip("Anterior").on_click(cx.listener(|h, _, _, cx| h.step(false, cx))))
                    .child(Button::new("hero-next").ghost().small().icon(IconName::ChevronRight).tooltip("Próximo").on_click(cx.listener(|h, _, _, cx| h.step(true, cx)))),
            )
            .into_any_element()
    }
}

impl Render for Hero {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let (items, done) = self.slots.get(&self.selected).map_or((Vec::new(), false), |s| (s.items.clone(), s.done));
        let current = items.get(self.index.get()).cloned();

        // Pré-carrega o banner do próximo slide: a troca não mostra fundo vazio.
        if items.len() > 1 {
            let next = &items[(self.index.get() + 1) % items.len()].0;
            if let Some(url) = next.summary.banner_image.clone() {
                self.state.update(cx, |s, cx| {
                    s.image(&url, cx);
                });
            }
        }

        let selected = self.selected;
        let header = h_flex()
            .h(px(HEADER_REM * rem))
            .justify_between()
            .gap_4()
            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(format!("Destaques de {}", current_year())))
            .child(
                ButtonGroup::new("hero-media")
                    .outline()
                    .small()
                    .child(KitButton::new("hero-anime").label("Anime").selected(selected == MediaType::Anime))
                    .child(KitButton::new("hero-manga").label("Mangá").selected(selected == MediaType::Manga))
                    .on_click(cx.listener(|h, picked: &Vec<usize>, _, cx| {
                        if let Some(&i) = picked.first() {
                            h.select(if i == 0 { MediaType::Anime } else { MediaType::Manga }, cx);
                        }
                    })),
            );

        let body: gpui_kit::AnyElement = match current {
            Some((m, path)) => self.slide(&m, &path, rem, cx),
            None if !done => Skeleton::new().size_full().into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(empty_state(
                    IconName::Inbox,
                    "Nenhum destaque do ano no acervo ainda",
                    Some("As obras chegam ao acervo na temporada seguinte.".into()),
                    cx,
                ))
                .into_any_element(),
        };
        let controls = (items.len() > 1).then(|| self.controls(&items, cx));

        v_flex().size_full().gap(px(GAP_REM * rem)).child(header).child(
            div()
                .id("hero-card")
                .relative()
                .h(px(CARD_REM * rem))
                .w_full()
                .rounded(theme.radius_lg)
                .overflow_hidden()
                .bg(theme.group_box)
                .on_hover(cx.listener(|h, hovered: &bool, _, _| h.hovered = *hovered))
                .child(body)
                // Borda por cima: a imagem do slide (absoluta) cobriria a borda do cartão.
                .child(div().absolute().inset_0().rounded(theme.radius_lg).border_1().border_color(theme.border))
                .children(controls),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_index_wraps_forward_and_back() {
        let mut s = SlideIndex::default();
        s.set_len(6);
        s.go(5);
        s.next();
        assert_eq!(s.get(), 0);
        s.prev();
        assert_eq!(s.get(), 5);
    }

    #[test]
    fn slide_index_shrinking_len_clamps() {
        let mut s = SlideIndex::default();
        s.set_len(6);
        s.go(5);
        s.set_len(3);
        assert_eq!(s.get(), 2);
    }

    #[test]
    fn slide_index_empty_len_is_zero() {
        let mut s = SlideIndex::default();
        s.next();
        s.prev();
        s.go(4);
        assert_eq!(s.get(), 0);
    }
}
