//! Banner de destaques do ano na entrada, sobre o `Carousel` do kit: um slide
//! largo por vez (arrastar, teclado e laço vêm do kit), miniaturas das capas como
//! paginação, texto que entra em cascata a cada troca e o fundo tingido com a cor
//! da capa. Alterna Anime/Mangá; cada tipo só confere quando é escolhido.

use std::{collections::HashMap, sync::Arc, time::Duration};

use gpui_kit::{
    Animation, AnimationExt as _, Context, ElementId, Entity, FontWeight, Hsla, IntoElement, ParentElement, Render, Styled,
    Subscription, Task, Window, assets::IconName, div, prelude::*, px, relative,
};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::ButtonVariants as _,
    carousel::{Carousel, CarouselContent, CarouselEvent, CarouselItem, CarouselPagination, CarouselPaginationItem, CarouselState},
    h_flex,
    skeleton::Skeleton,
    tab::{Tab, TabBar},
    tag::Tag,
    v_flex,
};
use higanbana_core::{
    anilist::{MediaType, Ranking, current_year, text, types::RankedMedia},
    catalog::ListBudget,
};

use super::{Slot, discover_task, eyebrow, leave};
use crate::{
    components::{cover, hex_color, remote_image, rounded_image},
    interact::Button,
    motion::{self, RISE_DISTANCE},
    state::{AppState, state},
    ui::empty_state,
};

/// Obras no banner.
const SLIDES: usize = 6;
/// Tempo de cada slide sem interação.
const AUTO_ADVANCE: Duration = Duration::from_secs(8);
/// Alturas em rem: cabeçalho, espaço e cartão.
const CARD_REM: f32 = 24.;
const HEADER_REM: f32 = 2.5;
const GAP_REM: f32 = 1.;
/// Quanto o banner desliza durante um slide (fração da largura).
const DRIFT: f32 = 0.05;

/// Altura total do bloco (para o `cached` de quem embute).
pub fn height_rem() -> f32 {
    HEADER_REM + GAP_REM + CARD_REM
}

/// Cor ambiente do slide: a cor dominante da capa ou o tom primário do tema.
fn glow_of(m: Option<&RankedMedia>, fallback: Hsla) -> Hsla {
    m.and_then(|m| hex_color(m.summary.cover_image.as_ref().and_then(|c| c.color.as_deref()))).unwrap_or(fallback)
}

pub struct Hero {
    state: Entity<AppState>,
    budget: Arc<ListBudget>,
    selected: MediaType,
    slots: HashMap<MediaType, Slot>,
    carousel: Entity<CarouselState>,
    /// Muda a cada troca de slide: reinicia a entrada em cascata e o deslize.
    reveal: u64,
    hovered: bool,
    _timer: Task<()>,
    _carousel_events: Subscription,
}

impl Hero {
    pub fn new(budget: Arc<ListBudget>, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Imagens (banner, capa) chegam pelo AppState.
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let carousel = cx.new(|_| CarouselState::new(0).with_looping(true));
        let events = cx.subscribe(&carousel, |h, _, _: &CarouselEvent, cx| {
            h.reveal += 1;
            cx.notify();
        });
        let timer = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(AUTO_ADVANCE).await;
                let alive = this.update(cx, |h, cx| {
                    if !h.hovered && h.carousel.read(cx).item_count() > 1 {
                        h.carousel.update(cx, |c, cx| {
                            c.select_next(cx);
                        });
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
            carousel,
            reveal: 0,
            hovered: false,
            _timer: timer,
            _carousel_events: events,
        };
        hero.load(MediaType::Anime, cx);
        hero
    }

    /// O tipo mostrado já terminou de conferir (o orçamento fica livre para os outros blocos).
    pub fn settled(&self) -> bool {
        self.slots.get(&self.selected).is_some_and(|s| s.done)
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
                    let len = slot.items.len();
                    if h.selected == media {
                        h.carousel.update(cx, |c, cx| c.set_item_count(len, cx));
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
        self.slots.insert(media, Slot::new(task));
    }

    fn select(&mut self, media: MediaType, cx: &mut Context<Self>) {
        if self.selected == media {
            return;
        }
        leave(&mut self.slots, &self.selected);
        self.selected = media;
        self.load(media, cx);
        let len = self.slots.get(&media).map_or(0, |s| s.items.len());
        self.carousel.update(cx, |c, cx| {
            c.set_item_count(len, cx);
            c.set_selected_index(0, cx);
        });
        self.reveal += 1;
        cx.notify();
    }

    fn slide(&self, m: &RankedMedia, path: &str, active: bool, rem: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let s = &m.summary;
        let title = s.title();
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
        let key = format!("{}:{}:{}", self.selected.as_str(), s.id, self.reveal);

        // Cada linha do texto sobe e aparece um pouco depois da anterior (só no slide ativo).
        let rise = |el: gpui_kit::AnyElement, step: usize| -> gpui_kit::AnyElement {
            if !active {
                return el;
            }
            div()
                .relative()
                .child(el)
                .with_animation(ElementId::Name(format!("hero-in:{key}:{step}").into()), motion::staggered(step), |d, t| {
                    d.opacity(t).top(px(RISE_DISTANCE * (1. - t)))
                })
                .into_any_element()
        };

        let banner = s.banner_image.clone().or_else(|| s.cover().map(str::to_owned));
        let has_banner = s.banner_image.is_some();
        let backdrop = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(relative(1. + DRIFT))
            .opacity(if has_banner { 0.65 } else { 0.3 })
            .children(banner.map(|url| remote_image(Some(&url), cx)));
        // O banner desliza devagar durante o slide: a cena não fica parada.
        let backdrop = if active {
            backdrop
                .with_animation(ElementId::Name(format!("hero-drift:{key}").into()), Animation::new(AUTO_ADVANCE).with_easing(motion::ease_out_cubic), move |d, t| {
                    d.left(relative(-DRIFT * t))
                })
                .into_any_element()
        } else {
            backdrop.into_any_element()
        };

        let title_el = div().text_3xl().line_height(relative(1.1)).font_weight(FontWeight::BOLD).line_clamp(2).child(title.clone()).into_any_element();
        let meta_el = h_flex()
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
            .when(!meta.is_empty(), |d| d.child(meta.join(" · ")))
            .into_any_element();
        let chips_el = h_flex().flex_wrap().gap_1p5().children(chips.into_iter().map(|c| Tag::secondary().small().child(c))).into_any_element();
        let synopsis_el =
            div().text_sm().line_height(relative(1.55)).text_color(theme.foreground.opacity(0.85)).line_clamp(3).child(synopsis.clone()).into_any_element();
        let open_el = h_flex()
            .pt_1()
            .child(
                Button::new(ElementId::Name(format!("hero-open:{}", s.id).into()))
                    .icon(IconName::ChevronRight)
                    .label("Abrir")
                    .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx))),
            )
            .into_any_element();

        let cover_el = div()
            .flex_none()
            .shadow_2xl()
            .rounded(theme.radius_lg)
            .child(cover(Some(s), &title, Some(px(11. * rem)), cx))
            .into_any_element();

        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .child(backdrop)
            // Véus do tema: texto legível sobre qualquer banner, claro ou escuro.
            .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                90.,
                gpui_kit::linear_color_stop(theme.background.opacity(0.95), 0.2),
                gpui_kit::linear_color_stop(theme.background.opacity(0.), 0.9),
            )))
            .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                180.,
                gpui_kit::linear_color_stop(theme.background.opacity(0.), 0.45),
                gpui_kit::linear_color_stop(theme.background.opacity(0.75), 1.),
            )))
            .child(
                h_flex()
                    .absolute()
                    .inset_0()
                    .px_10()
                    .pb_8()
                    .gap_8()
                    .child(rise(cover_el, 0))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .max_w(px(40. * rem))
                            .gap_3()
                            .child(rise(title_el, 1))
                            .child(rise(meta_el, 2))
                            .child(rise(chips_el, 3))
                            .when(!synopsis.is_empty(), |d| d.child(rise(synopsis_el, 4)))
                            .child(rise(open_el, 5)),
                    ),
            )
            .into_any_element()
    }

    /// Paginação com miniaturas das capas: a selecionada acende e cresce.
    fn pagination(&self, items: &[(RankedMedia, String)], active: usize, rem: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let media = self.selected.as_str();
        CarouselPagination::new()
            .accessibility_label("Destaques")
            .gap_1p5()
            .children(items.iter().enumerate().map(|(i, (m, _))| {
                let on = i == active;
                let url = m.summary.cover_small().map(str::to_owned);
                CarouselPaginationItem::new(ElementId::Name(format!("hero-thumb:{media}:{}", m.summary.id).into()), i, &self.carousel)
                    .accessibility_label(m.summary.title())
                    .p_0()
                    .h_auto()
                    .child(
                        div()
                            .w(px(2.4 * rem))
                            .h(px(3.4 * rem))
                            .rounded(theme.radius)
                            .overflow_hidden()
                            .border_2()
                            .border_color(if on { theme.foreground } else { theme.foreground.opacity(0.) })
                            .opacity(if on { 1. } else { 0.55 })
                            .child(rounded_image(url.as_deref(), Some(theme.radius), cx))
                            .with_animation(
                                ElementId::Name(format!("hero-thumb-pop:{media}:{}:{on}", m.summary.id).into()),
                                motion::enter(motion::QUICK),
                                move |d, t| if on { d.relative().top(px(-3. * t)) } else { d },
                            ),
                    )
            }))
            .into_any_element()
    }
}

impl Render for Hero {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let (items, done) = self.slots.get(&self.selected).map_or((Vec::new(), false), |s| (s.items.clone(), s.done));
        let active = self.carousel.read(cx).selected_index().unwrap_or(0).min(items.len().saturating_sub(1));
        let glow = glow_of(items.get(active).map(|(m, _)| m), theme.primary);

        let selected = self.selected;
        let header = h_flex()
            .h(px(HEADER_REM * rem))
            .justify_between()
            .gap_4()
            .child(eyebrow(IconName::Sparkles, format!("Destaques de {}", current_year()), cx))
            .child(
                TabBar::new("hero-media")
                    .segmented()
                    .small()
                    .selected_index(if selected == MediaType::Anime { 0 } else { 1 })
                    .on_click(cx.listener(|h, ix: &usize, _, cx| h.select(if *ix == 0 { MediaType::Anime } else { MediaType::Manga }, cx)))
                    .child(Tab::new().label("Anime"))
                    .child(Tab::new().label("Mangá")),
            );

        let body: gpui_kit::AnyElement = if items.is_empty() {
            if done {
                div()
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
                    .into_any_element()
            } else {
                Skeleton::new().size_full().into_any_element()
            }
        } else {
            let slides: Vec<_> = items
                .iter()
                .enumerate()
                .map(|(i, (m, path))| {
                    CarouselItem::new(ElementId::Name(format!("hero-slide:{}:{}", selected.as_str(), m.summary.id).into()), i, &self.carousel)
                        .accessibility_label(m.summary.title())
                        .pl_0()
                        .h_full()
                        .child(self.slide(m, path, i == active, rem, cx))
                })
                .collect();
            Carousel::new("hero-carousel", &self.carousel)
                .accessibility_label("Destaques do ano")
                .size_full()
                .child(CarouselContent::new(&self.carousel).h(px(CARD_REM * rem - 2.)).track_style(gpui_kit::StyleRefinement::default().ml_0()).children(slides))
                .into_any_element()
        };

        let controls = (items.len() > 1).then(|| {
            h_flex()
                .absolute()
                .right_6()
                .bottom_5()
                .gap_3()
                .child(self.pagination(&items, active, rem, cx))
                .child(
                    v_flex()
                        .gap_1()
                        .child(Button::new("hero-prev").ghost().small().icon(IconName::ChevronLeft).tooltip("Anterior").on_click(cx.listener(|h, _, _, cx| {
                            h.carousel.update(cx, |c, cx| {
                                c.select_previous(cx);
                            })
                        })))
                        .child(Button::new("hero-next").ghost().small().icon(IconName::ChevronRight).tooltip("Próximo").on_click(cx.listener(|h, _, _, cx| {
                            h.carousel.update(cx, |c, cx| {
                                c.select_next(cx);
                            })
                        }))),
                )
        });

        // Moldura de 1px por fora: o Carousel pinta por cima do que fica dentro dele.
        v_flex().size_full().gap(px(GAP_REM * rem)).child(header).child(
            div().h(px(CARD_REM * rem)).w_full().p(px(1.)).rounded(theme.radius_lg).bg(theme.border).child(
            div()
                .id("hero-card")
                .relative()
                .size_full()
                .rounded(theme.radius_lg - px(1.))
                .overflow_hidden()
                .bg(theme.group_box)
                .on_hover(cx.listener(|h, hovered: &bool, _, _| h.hovered = *hovered))
                // Luz ambiente na cor da capa, trocada com um fade a cada slide.
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(gpui_kit::linear_gradient(
                            135.,
                            gpui_kit::linear_color_stop(glow.opacity(0.35), 0.),
                            gpui_kit::linear_color_stop(glow.opacity(0.), 0.7),
                        ))
                        .with_animation(ElementId::Name(format!("hero-glow:{}", self.reveal).into()), motion::enter(motion::BASE), |d, t| d.opacity(t)),
                )
                .child(body)
                .children(controls),
            ),
        )
    }
}
