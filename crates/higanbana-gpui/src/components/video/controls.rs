//! Barra de controles: progresso, play/pausa, tempo, volume, velocidade, faixas e tela cheia.
use super::{controls_id, fmt_time, part, Lens, VideoState};
use crate::color::{rgba, Color};
use crate::components::{Component, Select, Slider};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId, MouseEvent};
use crate::style::Styled;
use crate::video::{PlayerState, Status, Track};

const ICON_FONT: &str = "Segoe MDL2 Assets";
const PLAY: &str = "\u{E768}";
const PAUSE: &str = "\u{E769}";
const VOLUME: &str = "\u{E767}";
const MUTED: &str = "\u{E74F}";
const FULLSCREEN: &str = "\u{E740}";
const EXIT_FULLSCREEN: &str = "\u{E73F}";
const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
const WHITE: Color = rgba(0xffff_ffff);

fn icon_button<V: 'static>(id: ElementId, glyph: &'static str, f: impl Fn(&mut V, &MouseEvent, &mut EventCx) + 'static) -> Element<V> {
    div()
        .id(id)
        .size(36.0)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded(6.0)
        .cursor_pointer()
        .hover(|s| s.bg(rgba(0xffff_ff26)))
        .on_click(f)
        .child(text(glyph).font_family(ICON_FONT).text_size(15.0))
}

/// Menu de faixas com "Desligado(a)" na primeira posição.
fn track_select<V: 'static>(
    id: ElementId,
    state: crate::components::SelectState,
    off: &'static str,
    tracks: &[Track],
    current: Option<i64>,
    lens: &Lens<V>,
    pick: fn(&VideoState, Option<i64>),
    which: fn(&mut VideoState) -> &mut crate::components::SelectState,
    cx: &mut RenderCx<'_, V>,
) -> Element<V> {
    let mut s = state;
    s.selected = Some(current.and_then(|c| tracks.iter().position(|t| t.id == c).map(|i| i + 1)).unwrap_or(0));
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    let labels = std::iter::once(off.to_string()).chain(tracks.iter().map(Track::label));
    let (l1, l2) = (lens.clone(), lens.clone());
    Select::new(id, &s, move |v: &mut V| which(l1(v)))
        .items(labels)
        .width(150.0)
        .on_change(move |v: &mut V, i, cx| {
            pick(l2(v), if i == 0 { None } else { ids.get(i - 1).copied() });
            cx.notify();
        })
        .render(cx)
}

pub(super) fn bar<V: 'static>(id: ElementId, lens: &Lens<V>, state: &mut VideoState, st: &PlayerState, cx: &mut RenderCx<'_, V>) -> Element<V> {
    let fullscreen = cx.window_state().fullscreen;
    let l = lens.clone();
    let seek = Slider::new(part(id, "seek"), st.position as f32)
        .range(0.0, st.duration.max(0.1) as f32)
        .step(0.0)
        .on_change(move |v: &mut V, x, _| l(v).player.seek_preview(x as f64))
        .render(cx);
    let l = lens.clone();
    let volume = Slider::new(part(id, "volume"), st.volume as f32)
        .width(96.0)
        .on_change(move |v: &mut V, x, _| l(v).player.set_volume(x as f64))
        .render(cx);

    let mut speed = state.speed;
    speed.selected = SPEEDS.iter().position(|s| (s - st.speed).abs() < 0.01);
    let l = lens.clone();
    let lsel = lens.clone();
    let speed = Select::new(part(id, "velocidade"), &speed, move |v: &mut V| &mut lsel(v).speed)
        .items(SPEEDS.iter().map(|s| format!("{s}x")))
        .width(90.0)
        .on_change(move |v: &mut V, i, cx| {
            l(v).player.set_speed(SPEEDS[i]);
            cx.notify();
        })
        .render(cx);
    let audio = (st.audio_tracks.len() > 1).then(|| {
        track_select(part(id, "audio"), state.audio, "Sem áudio", &st.audio_tracks, st.audio, lens,
            |s, t| s.player.set_audio_track(t), |s| &mut s.audio, cx)
    });
    let subs = (!st.sub_tracks.is_empty()).then(|| {
        track_select(part(id, "legenda"), state.sub, "Sem legenda", &st.sub_tracks, st.sub, lens,
            |s, t| s.player.set_sub_track(t), |s| &mut s.sub, cx)
    });

    let (l1, l2) = (lens.clone(), lens.clone());
    let play_glyph = if st.status == Status::Playing { PAUSE } else { PLAY };
    let play = icon_button(part(id, "play"), play_glyph, move |v: &mut V, _, cx| {
        l1(v).player.toggle_pause();
        cx.notify();
    });
    let mute = icon_button(part(id, "mudo"), if st.muted { MUTED } else { VOLUME }, move |v: &mut V, _, cx| {
        l2(v).player.toggle_mute();
        cx.notify();
    });
    let full_glyph = if fullscreen { EXIT_FULLSCREEN } else { FULLSCREEN };
    let full = icon_button(part(id, "tela-cheia"), full_glyph, |_: &mut V, _, cx| {
        cx.window_command(crate::gfx::WindowCommand::ToggleFullscreen);
    });
    let time = text(format!("{} / {}", fmt_time(st.position), fmt_time(st.duration))).text_size(13.0).whitespace_nowrap();

    let row = div()
        .items_center()
        .gap(8.0)
        .child(play)
        .child(mute)
        .child(div().w(96.0).mr(12.0).flex_shrink_0().child(volume))
        .child(time)
        .child(div().flex_1())
        .child(speed)
        .children(audio)
        .children(subs)
        .child(full);
    div()
        .id(controls_id(id))
        .absolute()
        .left(0.0)
        .right(0.0)
        .bottom(0.0)
        .flex_col()
        .gap(6.0)
        .px(16.0)
        .pt(24.0)
        .pb(12.0)
        .bg(rgba(0x0000_00a6))
        .text_color(WHITE)
        .on_click(|_: &mut V, _, cx| cx.stop_propagation())
        .child(seek)
        .child(row)
}
