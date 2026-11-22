//! Faixas por disco: número (ou barrinhas tocando), título, artista e duração.

use std::f32::consts::TAU;
use std::rc::Rc;
use std::time::Duration;

use higanbana_core::music::album::{format_duration, Album};
use higanbana_gpui::prelude::*;

use super::disc_groups;
use crate::root::Higanbana;

/// Barrinhas de "tocando": sobem e descem em ritmos diferentes, a 15 quadros por segundo.
fn playing_bars(cx: &mut RenderCx<'_, Higanbana>, color: Color) -> Element<Higanbana> {
    let secs = cx.now().duration_since(*START.get_or_init(std::time::Instant::now)).as_secs_f32();
    cx.request_redraw_in(Duration::from_millis(66));
    let mut row = div().items_end().gap(2.0).h(16.0);
    for i in 0..3u32 {
        let period = 0.76 + i as f32 * 0.17;
        let k = 0.65 + 0.35 * (secs / period * TAU).sin();
        row = row.child(div().w(3.0).h(16.0 * k).rounded_full().bg(color));
    }
    row
}

static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn pad(n: Option<u32>, i: usize) -> String {
    format!("{:02}", n.unwrap_or(i as u32 + 1))
}

pub fn tracks(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, a: &Rc<Album>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let ink = crate::theme::primary_ink(&t);
    let current = v.state.music.is_current_album(a);
    let live = v.state.music.player.clone().filter(|_| current);
    let (playing, index) = (live.as_ref().is_some_and(|l| l.playing), live.as_ref().and_then(|l| l.index));
    let groups = disc_groups(a);
    let multi = groups.len() > 1;
    let mut col = div().flex_col().gap(16.0);
    for (disc, idx) in groups {
        let mut d = div().flex_col();
        if multi {
            d = d.child(div().mb(4.0).px(12.0).gap(8.0).items_center().text_size(t.typography.xs).text_color(t.muted_foreground).child(icon("disc-3").size(12.0)).child(text(format!("Disco {disc}"))));
        }
        for i in idx {
            let tr = &a.tracks[i];
            let meta = v.state.music.metas.get(&tr.path);
            let on = index == Some(i);
            let hover = t.list_hover;
            let num = if on && playing { playing_bars(cx, ink) } else { text(pad(tr.number, i)) };
            let title = meta.and_then(|m| m.title.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| tr.title.clone());
            let mut info = div().flex_1().min_w(0.0).flex_col().child(text(title).line_clamp(1).text_size(t.typography.base).text_color(if on { ink } else { t.foreground }));
            if let Some(ar) = meta.and_then(|m| m.artist.clone()) {
                info = info.child(text(ar).line_clamp(1).text_size(t.typography.xs).text_color(t.muted_foreground));
            }
            let dur = meta.and_then(|m| m.duration).map(format_duration).unwrap_or_default();
            let mut row = div().id(("faixa", tr.path.as_str())).items_center().gap(12.0).px(12.0).py(10.0).rounded(t.radius).cursor_pointer();
            row = if on { row.bg(t.list_active) } else { row.hover(move |s| s.bg(hover)) };
            let al = a.clone();
            row = row
                .child(div().w(40.0).flex_none().text_size(t.typography.lg).font_weight(700).text_color(if on { ink } else { t.muted_foreground }).child(num))
                .child(info)
                .child(div().w(48.0).justify_end().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(dur)))
                .on_click(move |v: &mut Higanbana, _, cx| {
                    if on {
                        let _ = v.state.core.music().command("toggle");
                    } else {
                        v.state.play_album(al.clone(), i, 0.);
                    }
                    cx.notify();
                });
            d = d.child(row);
        }
        col = col.child(d);
    }
    col
}
