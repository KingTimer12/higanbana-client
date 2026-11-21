//! Banner de destaques do ano: um slide largo por vez, auto-avanço de 8 s
//! (pausa com o mouse em cima), miniaturas das capas como paginação e luz
//! ambiente na cor da capa. Só anime (o ranking de mangás quase não tem acervo).

use std::time::Duration;

use higanbana_core::anilist::{current_year, types::RankedMedia, MediaType, Ranking};
use higanbana_gpui::components::{Carousel, Component, Skeleton};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};
use higanbana_gpui::ElementId;

use super::eyebrow;
use super::hero_slide::slide;
use crate::motion::{ease_out_cubic, transition, BASE};
use crate::root::Higanbana;
use crate::ui::image::{hex_color, remote_image};
use crate::ui::{empty_state, hover, SHADOW_LG};

/// Obras no banner.
pub const SLIDES: usize = 6;
/// Tempo de cada slide sem interação.
pub const AUTO_ADVANCE: Duration = Duration::from_secs(8);
/// Alturas: cabeçalho, espaço e cartão.
const CARD: f32 = 384.0;
const HEADER: f32 = 40.0;

/// Cor ambiente do slide: a cor dominante da capa ou o tom primário do tema.
pub fn glow_of(m: Option<&RankedMedia>, fallback: Color) -> Color {
    m.and_then(|m| hex_color(m.summary.color())).unwrap_or(fallback)
}

/// Miniaturas das capas: a atual fica acesa e levantada; o hover levanta as outras.
fn thumbs(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, items: &[(RankedMedia, String)], active: usize) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let mut row = div().gap(8.0).items_end();
    for (i, (m, _)) in items.iter().enumerate() {
        let on = i == active;
        let id = ElementId::from(("hero-capa", m.summary.id as usize));
        let h = hover::amount(cx, id);
        let lift = if on { 1.0 } else { ease_out_cubic(h) };
        let url = m.summary.cover_small().map(str::to_owned);
        let mut thumb = div()
            .w(44.0)
            .h(62.4)
            .translate_y(-4.0 * lift)
            .rounded(t.radius)
            .overflow_hidden()
            .border(2.0, if on { t.foreground } else { t.foreground.alpha(0.2 * h) })
            .opacity(if on { 1.0 } else { 0.5 + 0.4 * h })
            .cursor_pointer()
            .child(remote_image(v, cx, url.as_deref(), t.radius));
        if on {
            thumb = thumb.shadow(Color::BLACK.alpha(0.25), SHADOW_LG);
        }
        let thumb = thumb.on_click(move |v: &mut Higanbana, _, cx| {
            v.pages.home().hero.go(i, std::time::Instant::now());
            cx.notify();
        });
        row = row.child(hover::track(thumb, id));
    }
    row
}

pub fn hero(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let now = cx.now();
    let (items, done) = v.state.home.slot(MediaType::Anime, Ranking::Popular).map_or((Vec::new(), false), |s| (s.items.clone(), s.done));
    // Pausa com o mouse em cima; o total acompanha o que já chegou.
    let hovered = cx.is_hovered("hero-cartao");
    let ui = v.pages.home();
    if ui.hero.len() != items.len() {
        ui.hero.set_len(items.len(), now);
    }
    ui.hero.set_paused(hovered, now);
    let state = ui.hero.clone();
    let active = state.current(now).min(items.len().saturating_sub(1));
    let glow = glow_of(items.get(active).map(|(m, _)| m), t.primary);
    let header = div().h(HEADER).items_center().child(eyebrow(&t, "sparkles", format!("Destaques de {}", current_year())));
    let body = if items.is_empty() {
        if done {
            div().size_full().items_center().justify_center().child(empty_state(&t, "inbox", "Nenhum destaque do ano no acervo ainda", Some("As obras chegam ao acervo na temporada seguinte.".into())))
        } else {
            Skeleton::new("hero-esqueleto").w(pct(100.0)).h(pct(100.0)).render(cx)
        }
    } else {
        let mut slides = Vec::with_capacity(items.len());
        for (i, (m, path)) in items.iter().enumerate() {
            slides.push(slide(v, cx, m, path, i == active, CARD - 2.0));
        }
        Carousel::new("hero-carrossel", &state, |v: &mut Higanbana| &mut v.pages.home().hero).items(slides).render(cx)
    };
    // Luz ambiente na cor da capa, trocada com um fade a cada slide.
    let g = cx.animate_from(("hero-luz", active), 0.0, 1.0, transition(BASE));
    let light = div().absolute().top(0.0).left(0.0).size_full().opacity(g).bg_gradient(LinearGradient::new(GradientDir::Right, vec![(0.0, glow.alpha(0.35)), (0.7, glow.alpha(0.0))]));
    let mut card = div().relative().size_full().rounded(t.radius_lg - 1.0).overflow_hidden().bg(t.group_box).child(light).child(body);
    if items.len() > 1 {
        card = card.child(div().absolute().right(24.0).bottom(20.0).child(thumbs(v, cx, &items, active)));
    }
    // Moldura de 1 px por fora.
    let frame = div().h(CARD).w_full().p(1.0).rounded(t.radius_lg).bg(t.border).child(hover::track(card, "hero-cartao"));
    div().w_full().flex_col().gap(16.0).child(header).child(frame)
}
