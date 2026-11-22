//! Barra de posição com prévia (tempo, ou miniatura + tempo) acima do mouse.

use higanbana_core::player::logic::format_time;
use higanbana_gpui::components::{Component, Seekbar};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ObjectFit;

use super::style::SHADOW_MD;
use crate::root::Higanbana;
use crate::theme::media_scrim;

/// Prévia do quadro (16:9).
pub const THUMB_W: f32 = 176.0;
pub const THUMB_H: f32 = 99.0;

/// `left` da prévia com miniatura: centrada no mouse, sem passar das pontas.
pub fn preview_left(x: f32, bar_w: f32) -> f32 {
    (x - THUMB_W / 2.0).min(bar_w - THUMB_W).max(0.0)
}

pub fn seekbar(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let Some(p) = v.player.as_mut() else { return div() };
    let t = cx.theme().clone();
    let (dur, pos) = (p.session.dur(), p.last.position);
    let marks = p.last.chapters.iter().map(|c| c.time).collect();
    let thumbs_on = p.session.settings().thumbnails;
    // Pede a miniatura do ponto em hover (abre o gerador no primeiro pedido).
    let hover_t = p.seek.hover.filter(|_| p.seek.width > 0.0).map(|x| (x / p.seek.width) as f64 * dur);
    if let (Some(ht), true) = (hover_t, thumbs_on) {
        p.want_thumb(ht);
    }
    let frame = hover_t.and_then(|ht| p.thumb(ht));
    let state = p.seek;
    Seekbar::new("player-barra", &state, |v: &mut Higanbana| super::seek_of(&mut v.player, &mut v.player_spare))
        .duration(dur)
        .position(pos)
        .buffered(pos + p.last.cache_ahead)
        .marks(marks)
        .played_color(t.primary)
        .hover_content(move |secs, x, w| {
            let time = div().text_size(t.typography.xs).font_weight(500).child(text(format_time(secs)));
            let Some(frame) = frame.clone() else {
                return div().absolute().bottom(0.0).left(x - 24.0).px(8.0).py(2.0).rounded(t.radius).bg(t.popover).child(time);
            };
            let pic = match frame {
                Some(src) => img(src).w(THUMB_W).h(THUMB_H).object_fit(ObjectFit::Cover).rounded(t.radius),
                None => div().w(THUMB_W).h(THUMB_H).rounded(t.radius).bg(media_scrim(0.6)),
            };
            div()
                .absolute()
                .bottom(0.0)
                .left(preview_left(x, w))
                .flex_col()
                .items_center()
                .gap(4.0)
                .p(4.0)
                .rounded(t.radius_lg)
                .bg(t.popover)
                .shadow(Color::BLACK.alpha(0.1), SHADOW_MD)
                .child(pic)
                .child(time.pb(2.0))
        })
        .on_seek(|v: &mut Higanbana, secs, cx| {
            if let Some(p) = v.player.as_mut() {
                let d = p.session.dur();
                p.player.seek(secs.min(d - 0.5).max(0.0));
            }
            cx.notify();
        })
        .render(cx)
}
