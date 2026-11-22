//! Coluna do encarte: capa com obi, título, ações (tocar/continuar) e formato.

use std::rc::Rc;

use higanbana_core::music::album::{format_duration, format_label, Album};
use higanbana_gpui::components::{Button, Component, ComponentSize, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ObjectFit;

use crate::root::Higanbana;
use crate::state::Img;

/// Lado da capa (14 rem).
const ART: f32 = 224.0;

pub fn aside(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, a: &Rc<Album>, work: Option<String>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let cover = v.state.album_cover(a);
    let m = &v.state.music;
    let current = m.is_current_album(a);
    let live = if current { m.player.clone() } else { None };
    let metas: Vec<_> = a.tracks.iter().map(|tr| m.metas.get(&tr.path)).collect();
    let format = live
        .as_ref()
        .and_then(|l| l.format.clone())
        .map(|f| format_label(&f.codec, f.bits, f.sample_rate, f.lossless, f.resampled_to))
        .or_else(|| metas.iter().flatten().next().map(|m| format_label(&m.codec, m.bits, m.sample_rate, m.lossless, None)));
    let durations: Vec<Option<f64>> = metas.iter().map(|m| m.and_then(|m| m.duration)).collect();
    let total = durations.iter().all(Option::is_some).then(|| durations.iter().flatten().sum::<f64>());
    let progress = v.state.core.music().store.data().progress.get(&a.path).copied();
    let resume = progress.filter(|p| (p.index > 0 || p.position > 5.) && p.index < a.tracks.len());
    let playing = live.as_ref().is_some_and(|l| l.playing);
    let ended = live.as_ref().is_some_and(|l| l.ended);

    let art = match cover {
        Img::Ready(i) => img(i).size_full().object_fit(ObjectFit::Cover),
        _ => div().size_full().items_center().justify_center().p(16.0).text_size(t.typography.lg).font_weight(700).text_color(t.muted_foreground).child(text(a.title.clone())),
    };
    let mut boxed = div().relative().w(ART + 20.0).child(div().w(ART).h(ART).overflow_hidden().rounded(t.radius).bg(t.muted).shadow(Color::BLACK.fade(0.25), 16.0).child(art));
    // Obi: a faixa vertical dos CDs japoneses.
    if let Some(o) = a.catalog.clone().or_else(|| a.year.clone()) {
        let mut obi = div().absolute().top(0.0).right(0.0).h_full().w(32.0).flex_col().items_center().justify_center().bg(t.primary).text_color(t.primary_foreground).text_size(t.typography.xs).font_weight(700);
        obi = obi.rounded_corners(higanbana_gpui::Corners { top_left: 0.0, bottom_left: 0.0, top_right: t.radius, bottom_right: t.radius });
        for c in o.chars() {
            obi = obi.child(text(c.to_string()));
        }
        boxed = boxed.child(obi);
    }

    let (label, ic) = if current && playing {
        ("Pausar", "pause")
    } else if current && !ended {
        ("Continuar", "play")
    } else {
        ("Tocar", "play")
    };
    let al = a.clone();
    let main = Button::new("album-tocar", label).size(ComponentSize::Sm).icon(icon(ic).size(16.0)).on_click(move |v: &mut Higanbana, _, cx| {
        if current && playing {
            let _ = v.state.core.music().command("pause");
        } else if current && !ended {
            let _ = v.state.core.music().command("resume");
        } else {
            v.state.play_album(al.clone(), 0, 0.);
        }
        cx.notify();
    });
    let mut actions = div().flex_wrap().gap(8.0).child(main.render(cx));
    if let Some(p) = resume.filter(|_| !current) {
        let al = a.clone();
        let b = Button::new("album-continuar", format!("Continuar faixa {} ({})", p.index + 1, format_duration(p.position))).variant(Variant::Outline).size(ComponentSize::Sm).on_click(move |v: &mut Higanbana, _, cx| {
            v.state.play_album(al.clone(), p.index, p.position);
            cx.notify();
        });
        actions = actions.child(b.render(cx));
    }
    let sub = [work, a.year.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let names = div().flex_col().gap(2.0).child(text(a.title.clone()).text_size(t.typography.xl).font_weight(700)).child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(sub)));
    let mut facts = div().flex_col().gap(2.0).text_size(t.typography.xs).text_color(t.muted_foreground);
    if let Some(f) = format {
        facts = facts.child(text(f));
    }
    facts = facts.child(text(format!("{} faixas{}", a.tracks.len(), total.map(|s| format!(" · {}", format_duration(s))).unwrap_or_default())));
    div().w(ART + 56.0).flex_none().flex_col().gap(16.0).p(16.0).rounded(t.radius_lg).border(1.0, t.border).bg(t.group_box).child(boxed).child(names).child(actions).child(facts)
}
