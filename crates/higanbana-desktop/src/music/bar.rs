//! Barra do player de músicas, embaixo da janela enquanto um álbum toca.

use higanbana_core::library::{base_name, parent_path};
use higanbana_core::music::album::{album_title, format_duration, parse_track};
use higanbana_gpui::components::{Button, Component, ComponentSize, Slider, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ObjectFit;

use super::session::repeat_cycle;
use crate::chrome::tip;
use crate::root::Higanbana;
use crate::state::Img;

/// Laterais iguais (17,5 rem) para os controles ficarem no centro da janela.
const SIDE: f32 = 280.0;

fn ghost(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: &'static str, name: &'static str, hint: &'static str, on: bool, f: impl Fn(&mut Higanbana) + 'static) -> Element<Higanbana> {
    let accent = cx.theme().accent;
    let b = Button::new(id, "").variant(Variant::Ghost).size(ComponentSize::Regular).icon(icon(name).size(16.0)).on_click(move |v: &mut Higanbana, _, cx| {
        f(v);
        cx.notify();
    });
    let b = b.render(cx);
    tip(v, cx, ("dica-musica", id), if on { b.bg(accent) } else { b }, hint)
}

pub fn bar(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(m) = v.state.music.player.clone() else { return div() };
    let Some(album_path) = m.album.clone() else { return div() };
    let meta = m.path.as_ref().and_then(|p| v.state.music.metas.get(p));
    let file_title = m.path.as_deref().map(|p| parse_track(base_name(p)).title).unwrap_or_default();
    let title = m.title.clone().or_else(|| meta.and_then(|x| x.title.clone())).filter(|s| !s.is_empty()).unwrap_or(file_title);
    let album_name = album_title(base_name(&album_path));
    let work = base_name(&parent_path(&album_path)).to_owned();
    let subtitle = m.artist.clone().unwrap_or_else(|| if work.is_empty() { album_name.clone() } else { format!("{work} · {album_name}") });
    let dur = m.duration.unwrap_or(0.);
    let shown = v.state.music.seeking.map_or(m.position, |f| f as f64 * dur);
    let frac = if dur > 0. { (shown / dur).clamp(0., 1.) as f32 } else { 0. };
    let muted = m.volume <= 0.001;
    let vol_icon = if muted { "volume-x" } else if m.volume < 0.5 { "volume-1" } else { "volume-2" };
    let (repeat_icon, repeat_tip, repeat_next) = repeat_cycle(m.repeat);

    // Faixa atual: capa, título e obra (abre o álbum).
    let cover = match v.state.cached_image(&format!("album-cover:{album_path}")) {
        Some(Img::Ready(i)) => img(i).size_full().object_fit(ObjectFit::Cover),
        _ => div(),
    };
    let hover = t.list_hover;
    let open = album_path.clone();
    let now = div()
        .id("musica-abrir-album")
        .w(SIDE)
        .flex_none()
        .items_center()
        .gap(12.0)
        .p(4.0)
        .rounded(t.radius)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(div().size(48.0).flex_none().rounded(t.radius).overflow_hidden().bg(t.muted).child(cover))
        .child(div().flex_col().min_w(0.0).child(text(title).line_clamp(1).text_size(t.typography.sm).font_weight(600)).child(text(subtitle).line_clamp(1).text_size(t.typography.xs).text_color(t.muted_foreground)))
        .on_click(move |v: &mut Higanbana, _, cx| {
            v.state.go(open.clone());
            cx.notify();
        });

    let (shuffle, normalize) = (m.shuffle, m.normalize);
    let toggle = Button::new("musica-tocar", "").size(ComponentSize::Md).icon(icon(if m.playing { "pause" } else { "play" }).size(16.0)).on_click(|v: &mut Higanbana, _, cx| {
        let _ = v.state.core.music().command("toggle");
        cx.notify();
    });
    let toggle = toggle.render(cx).rounded(t.radius_lg).w(36.0).px(0.0);
    let toggle = tip(v, cx, "dica-musica-tocar", toggle, if m.playing { "Pausar" } else { "Tocar" });
    let buttons = div()
        .gap(4.0)
        .items_center()
        .child(ghost(v, cx, "musica-aleatorio", "shuffle", if shuffle { "Aleatório ligado" } else { "Aleatório" }, shuffle, move |v| v.state.core.music().set(None, Some(!shuffle), None, None)))
        .child(ghost(v, cx, "musica-anterior", "skip-back", "Faixa anterior", false, |v| {
            let _ = v.state.core.music().command("prev");
        }))
        .child(toggle)
        .child(ghost(v, cx, "musica-proxima", "skip-forward", "Próxima faixa", false, |v| {
            let _ = v.state.core.music().command("next");
        }))
        .child(ghost(v, cx, "musica-repetir", repeat_icon, repeat_tip, m.repeat != higanbana_core::music::queue::Repeat::Off, move |v| v.state.core.music().set(None, None, Some(repeat_next), None)));
    let seek = Slider::new("musica-posicao", frac)
        .range(0.0, 1.0)
        .step(0.0)
        .width(higanbana_gpui::Length::Percent(100.0))
        .disabled(dur <= 0.)
        .on_change(|v: &mut Higanbana, f, cx| {
            v.state.music.seeking = Some(f);
            cx.notify();
        })
        .on_release(move |v: &mut Higanbana, f, cx| {
            v.state.music.seeking = None;
            v.state.core.music().seek(f as f64 * dur);
            cx.notify();
        })
        .render(cx);
    let times = div()
        .w_full()
        .max_w(560.0)
        .items_center()
        .gap(8.0)
        .text_size(t.typography.xs)
        .text_color(t.muted_foreground)
        .child(div().w(40.0).justify_end().child(text(format_duration(shown))))
        .child(div().flex_1().child(seek))
        .child(div().w(40.0).child(text(if dur > 0. { format_duration(dur) } else { "--:--".into() })));
    let center = div().flex_1().min_w(0.0).flex_col().items_center().gap(4.0).child(buttons).child(times);

    let norm = Button::new("musica-igualar", "Volume igualado").variant(Variant::Ghost).size(ComponentSize::Xs).on_click(move |v: &mut Higanbana, _, cx| {
        v.state.core.music().set(None, None, None, Some(!normalize));
        cx.notify();
    });
    let norm = norm.render(cx);
    let norm = if normalize { norm.bg(t.accent) } else { norm };
    let norm_tip = if normalize { "Volume igualado entre álbuns (vale a partir da próxima faixa)" } else { "Volume original dos arquivos (vale a partir da próxima faixa)" };
    let norm = tip(v, cx, "dica-musica-igualar", norm, norm_tip);
    let mute = ghost(v, cx, "musica-mudo", vol_icon, if muted { "Tirar do mudo" } else { "Mudo" }, false, |v| v.state.toggle_mute());
    let volume = Slider::new("musica-volume", m.volume)
        .range(0.0, 1.0)
        .step(0.01)
        .width(96.0)
        .on_change(|v: &mut Higanbana, f, cx| {
            v.state.core.music().set(Some(f), None, None, None);
            cx.notify();
        })
        .render(cx);
    let right = div().w(SIDE).flex_none().justify_end().items_center().gap(8.0).child(norm).child(mute).child(volume);

    let line = div().w_full().h(1.0).bg(t.border);
    let body = div().h(79.0).w_full().items_center().gap(16.0).px(16.0).bg(t.background).child(now).child(center).child(right);
    div().w_full().flex_none().flex_col().child(line).child(body)
}
