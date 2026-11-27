//! Seção do vidro líquido: palco com fundo animado (outro shader) e conteúdo nítido, três vidros
//! (um arrastável, dois animados) e sliders de refração, desfoque e tinta.
use super::liquid_glass::{liquid_glass, GlassParams};
use super::preview;
use crate::Gallery;
use higanbana_gpui::anim::{Easing, Repeat};
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;
use higanbana_gpui::point;

const STAGE_H: f32 = 440.0;

// Fundo animado do palco; traduzido uma vez por thread (o cache de GPU é por `Shader`).
thread_local! {
    static AURORA: Option<Shader> = Shader::wgsl(include_str!("glass_aurora.wgsl"))
        .map_err(|e| eprintln!("glass_aurora: {e}"))
        .ok()
        .map(|s| s.animated(true));
}

pub fn glass(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let stage = stage(v, cx);
    let controls = controls(v, cx);
    vec![preview(cx, "Arraste o cartão; os outros flutuam sozinhos", stage), preview(cx, "Ajustes", controls)]
}

fn params(v: &Gallery, radius: f32) -> GlassParams {
    GlassParams { refraction: v.glass_refraction, blur: v.glass_blur, tint: rgb(0xffe4ec).alpha(v.glass_tint), radius, ..GlassParams::default() }
}

/// Fundo + conteúdo nítido + vidros, nessa ordem (cada vidro refrata o que veio antes).
fn stage(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let swing = Transition::ms(4200).easing(Easing::EaseInOut).repeat(Repeat::Forever).yoyo();
    let bob = Transition::ms(2600).easing(Easing::EaseInOut).repeat(Repeat::Forever).yoyo();
    let x = cx.animate_from("vidro-pilula", 0.0, 1.0, swing);
    let y = cx.animate_from("vidro-lente", 0.0, 1.0, bob);
    let base = div().relative().w(780.0).h(STAGE_H).rounded(18.0).overflow_hidden();
    let base = match AURORA.with(|s| s.clone()) {
        Some(s) => base.shader(&s, params![]),
        None => base.bg(rgb(0x1a1030)),
    };
    base.child(grid())
        .child(lettering())
        .child(pill(v, x))
        .child(lens(v, y))
        .child(card(v))
}

/// Grade de linhas finas: bordas retas deixam a refração bem visível.
fn grid() -> Element<Gallery> {
    let line = |vertical: bool| {
        let l = div().flex_shrink_0().bg(Color::WHITE.alpha(0.16));
        if vertical { l.w(1.0).h_full() } else { l.h(1.0).w_full() }
    };
    let cols = div().absolute().inset(0.0).justify_between().px(20.0).children((0..19).map(|_| line(true)));
    let rows = div().absolute().inset(0.0).flex_col().justify_between().py(20.0).children((0..11).map(|_| line(false)));
    div().absolute().inset(0.0).child(cols).child(rows)
}

fn lettering() -> Element<Gallery> {
    div()
        .absolute()
        .inset(0.0)
        .flex_col()
        .items_center()
        .justify_center()
        .gap(6.0)
        .text_color(Color::WHITE)
        .child(text("彼岸花").text_size(112.0).font_bold())
        .child(text("H I G A N B A N A").text_size(22.0).font_weight(600).text_color(Color::WHITE.alpha(0.8)))
}

/// Cartão arrastável: guarda onde foi pego e segue o ponteiro (com captura durante o arraste).
fn card(v: &Gallery) -> Element<Gallery> {
    liquid_glass(params(v, 28.0))
        .id("vidro-cartao")
        .absolute()
        .left(v.glass_pos.x)
        .top(v.glass_pos.y)
        .w(300.0)
        .h(180.0)
        .p(22.0)
        .flex_col()
        .justify_between()
        .cursor(CursorStyle::Move)
        .text_color(Color::WHITE)
        .on_mouse_down(|v: &mut Gallery, ev, _| v.glass_grab = ev.local)
        .on_mouse_move(|v: &mut Gallery, ev, cx| {
            if !ev.pressed {
                return;
            }
            let x = v.glass_pos.x + ev.position.x - ev.bounds.origin.x - v.glass_grab.x;
            let y = v.glass_pos.y + ev.position.y - ev.bounds.origin.y - v.glass_grab.y;
            v.glass_pos = point(x.clamp(-150.0, 630.0), y.clamp(-90.0, STAGE_H - 90.0));
            cx.notify();
        })
        .child(text("Vidro líquido").text_size(22.0).font_bold())
        .child(text("Refração na borda, desfoque, tinta e um reflexo que gira.").text_size(14.0).text_color(Color::WHITE.alpha(0.85)))
}

/// Pílula que vai e volta na horizontal.
fn pill(v: &Gallery, t: f32) -> Element<Gallery> {
    liquid_glass(params(v, 32.0))
        .absolute()
        .left(40.0 + 460.0 * t)
        .top(STAGE_H - 96.0)
        .w(220.0)
        .h(64.0)
        .items_center()
        .justify_center()
        .gap(10.0)
        .text_color(Color::WHITE)
        .child(icon("play").size(18.0))
        .child(text("Episódio 12").font_weight(600))
}

/// Lente redonda que sobe e desce.
fn lens(v: &Gallery, t: f32) -> Element<Gallery> {
    liquid_glass(GlassParams { bevel: 40.0, ..params(v, 70.0) }).absolute().left(600.0).top(40.0 + 120.0 * t).size(140.0)
}

fn controls(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let muted = cx.theme().muted_foreground;
    let refr = Slider::new("vidro-refr", v.glass_refraction).range(0.0, 60.0).step(1.0).width(260.0).on_change(|v: &mut Gallery, x, _| v.glass_refraction = x).render(cx);
    // Desfoque até 20 px: limite recomendado do `backdrop_blur` (ver a limitação no prelude.wgsl).
    let blur = Slider::new("vidro-blur", v.glass_blur).range(0.0, 20.0).step(1.0).width(260.0).on_change(|v: &mut Gallery, x, _| v.glass_blur = x).render(cx);
    let tint = Slider::new("vidro-tinta", v.glass_tint).range(0.0, 0.6).step(0.02).width(260.0).on_change(|v: &mut Gallery, x, _| v.glass_tint = x).render(cx);
    let line = |label: &'static str, s: Element<Gallery>, value: String| {
        div()
            .flex_col()
            .gap(10.0)
            .child(div().w(260.0).justify_between().child(text(label).text_size(14.0).font_weight(500)).child(text(value).text_size(14.0).text_color(muted)))
            .child(s)
    };
    div()
        .flex_wrap()
        .gap(32.0)
        .child(line("Refração", refr, format!("{:.0} px", v.glass_refraction)))
        .child(line("Desfoque", blur, format!("{:.0} px", v.glass_blur)))
        .child(line("Tinta", tint, format!("{:.0}%", v.glass_tint * 100.0)))
}
