//! Controles de baixo: barra de posição e a linha de botões.

use higanbana_core::player::logic::{episode_label, format_time};
use higanbana_gpui::components::{Button, Component, Slider, Variant};
use higanbana_gpui::prelude::*;

use super::menus;
use super::seek::seekbar;
use super::style::scrim_up;
use crate::chrome::tip;
use crate::root::Higanbana;
use crate::theme::on_media_muted;
use crate::ui::truncate;

/// Ícone do volume como na master.
pub fn volume_icon(muted: bool, volume: f64) -> &'static str {
    if muted || volume == 0.0 {
        "volume-x"
    } else if volume < 50.0 {
        "volume-1"
    } else {
        "volume-2"
    }
}

/// Valor mostrado no slider: 0 quando mudo.
pub fn slider_value(muted: bool, volume: f64) -> f32 {
    if muted { 0.0 } else { volume as f32 }
}

/// "pos / dur".
pub fn time_label(pos: f64, dur: f64) -> String {
    format!("{} / {}", format_time(pos), format_time(dur))
}

fn ghost(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: &'static str, name: &'static str, hint: &'static str, disabled: bool, f: impl Fn(&mut Higanbana, &mut EventCx) + 'static) -> Element<Higanbana> {
    let b = Button::new(id, "")
        .variant(Variant::Ghost)
        .icon(icon(name).size(16.0))
        .disabled(disabled)
        .on_click(move |v: &mut Higanbana, _, cx| {
            f(v, cx);
            cx.notify();
        })
        .render(cx);
    tip(v, cx, format!("dica:{id}").as_str(), b, hint)
}

fn row(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let st = p.last.clone();
    let paused = st.status != higanbana_gpui::video::Status::Playing;
    let has_next = p.session.next().is_some();
    let label = episode_label(&p.session.target.file.name);
    let time = time_label(p.session.pos(), p.session.dur());
    let full = cx.window_state().fullscreen;
    let sm = cx.theme().typography.sm;
    let play = ghost(v, cx, "player-play", if paused { "play" } else { "pause" }, if paused { "Tocar (Espaço)" } else { "Pausar (Espaço)" }, false, |v, _| {
        if let Some(p) = v.player.as_ref() {
            p.player.toggle_pause();
        }
    });
    let next = ghost(v, cx, "player-proximo", "skip-forward", "Próximo episódio (N)", !has_next, |v, _| {
        if let Some(p) = v.player.as_mut() {
            let a = p.session.go_next(std::time::Instant::now());
            p.run(a);
        }
    });
    let mute = ghost(v, cx, "player-mudo", volume_icon(st.muted, st.volume), if st.muted { "Tirar do mudo (M)" } else { "Mudo (M)" }, false, |v, _| {
        if let Some(p) = v.player.as_ref() {
            p.player.toggle_mute();
        }
    });
    let slider = Slider::new("player-volume", slider_value(st.muted, st.volume))
        .range(0.0, 100.0)
        .step(1.0)
        .width(96.0)
        .on_change(|v: &mut Higanbana, n, cx| {
            if let Some(p) = v.player.as_ref() {
                p.player.set_volume(n as f64);
                if p.last.muted && n > 0.0 {
                    p.player.set_property("mute", "no");
                }
            }
            cx.notify();
        })
        .render(cx);
    let menus = menus::menus(v, cx);
    let fs = ghost(v, cx, "player-tela-cheia", if full { "minimize" } else { "maximize" }, if full { "Sair da tela cheia (F)" } else { "Tela cheia (F)" }, false, |_, cx| {
        cx.window_command(WindowCommand::ToggleFullscreen)
    });
    div()
        .items_center()
        .gap(4.0)
        .child(play)
        .child(next)
        .child(mute)
        .child(div().w(96.0).child(slider))
        .child(div().ml(8.0).flex_shrink_0().text_size(sm).text_color(on_media_muted()).child(text(time)))
        .child(div().flex_1().min_w(0.0).px(16.0).justify_center().text_size(sm).font_weight(500).child(truncate(label)))
        .children(menus)
        .child(fs)
}

pub fn controls(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, fade: f32) -> Element<Higanbana> {
    let bar = seekbar(v, cx);
    let row = row(v, cx);
    div()
        .id("player-controles")
        .flex_shrink_0()
        .flex_col()
        .gap(4.0)
        .px(20.0)
        .pt(40.0)
        .pb(12.0)
        .bg_gradient(scrim_up(0.85))
        .opacity(fade)
        .child(bar)
        .child(row)
}
