//! Seção de animações: presença com deslize, mola, spinner e cor em ciclo.
use super::preview;
use crate::Gallery;
use higanbana_gpui::anim::{Easing, Repeat, Spring, Transition};
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

const SLIDE: Transition = Transition::ms(220).easing(Easing::EaseOut);
const BOUNCY: Spring = Spring::new(320.0, 12.0);

pub fn anim(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let (a, b, c, d) = (presence(v, cx), spring(v, cx), spinner(cx), color_cycle(cx));
    vec![
        preview(cx, "Presença (entra e sai deslizando)", a),
        preview(cx, "Mola", b),
        preview(cx, "Repetição", c),
        preview(cx, "Cor em ciclo (ida e volta)", d),
    ]
}

fn presence(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let label = if v.anim_card { "Ocultar card" } else { "Mostrar card" };
    let toggle = Button::new("an-toggle", label)
        .variant(Variant::Outline)
        .on_click(|v: &mut Gallery, _, cx| {
            v.anim_card = !v.anim_card;
            cx.notify();
        })
        .render(cx);
    let theme = cx.theme();
    let card = cx.presence("an-card", v.anim_card, SLIDE).map(|p| {
        div()
            .w(260.0)
            .p(16.0)
            .flex_col()
            .gap(4.0)
            .bg(theme.card)
            .border(1.0, theme.border)
            .rounded(theme.radius)
            .opacity(p)
            .translate_y((1.0 - p) * 16.0)
            .scale(0.96 + 0.04 * p)
            .child(text("Episódio novo").font_weight(600))
            .child(text("Entra com fade, deslize e escala.").text_size(13.0).text_color(theme.muted_foreground))
    });
    div().flex_col().gap(12.0).h(120.0).child(toggle).children(card)
}

fn spring(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let s = cx.animate("an-mola", if v.anim_big { 1.25 } else { 1.0 }, Transition::with_spring(BOUNCY));
    let x = cx.animate("an-mola-x", if v.anim_big { 120.0 } else { 0.0 }, Transition::spring(170.0, 20.0));
    let theme = cx.theme();
    let ball = div().w(56.0).h(56.0).rounded_full().bg(theme.primary).translate_x(x).scale(s);
    let button = Button::new("an-mola-btn", "Empurrar")
        .on_click(|v: &mut Gallery, _, cx| {
            v.anim_big = !v.anim_big;
            cx.notify();
        })
        .render(cx);
    div().flex_col().gap(16.0).w(260.0).child(button).child(div().h(72.0).items_center().child(ball))
}

fn spinner(cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let color = cx.theme().primary;
    let mut row = div().gap(8.0).h(48.0).items_center();
    for i in 0..5 {
        let t = Transition::ms(500).easing(Easing::EaseInOut).delay(i as u64 * 90).repeat(Repeat::Forever).yoyo();
        let p = cx.animate_from(("an-dot", i), 0.0, 1.0, t);
        row = row.child(div().w(12.0).h(12.0).rounded_full().bg(color).translate_y(-14.0 * p).opacity(0.4 + 0.6 * p));
    }
    row
}

fn color_cycle(cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let theme = cx.theme();
    let t = Transition::ms(1600).easing(Easing::EaseInOut).repeat(Repeat::Forever).yoyo();
    let c = cx.animate_value_from("an-cor", theme.primary, theme.accent, t);
    div().w(220.0).h(64.0).rounded(theme.radius).bg(c).items_center().justify_center().child(text("Ciclo").text_color(theme.primary_foreground))
}
