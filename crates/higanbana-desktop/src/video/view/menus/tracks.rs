//! Menu "Legenda e áudio": legenda (com "Desligada") e áudio (só com mais de uma faixa).

use higanbana_core::player::logic::{Track, TrackKind};
use higanbana_core::player::store::TrackPref;
use higanbana_gpui::components::{Component, RadioGroup, Separator};
use higanbana_gpui::prelude::*;

use super::super::style::menu_heading;
use crate::root::Higanbana;
use crate::video::convert;

/// "{nome} ({LANG})" quando tem título e idioma; senão o nome do kit.
pub fn track_label(t: &Track) -> String {
    match (t.title.as_ref(), t.lang.as_ref()) {
        (Some(_), Some(lang)) => format!("{} ({})", t.name(), lang.to_uppercase()),
        _ => t.name(),
    }
}

fn choose(v: &mut Higanbana, t: Option<Track>, audio: bool) {
    let Some(p) = v.player.as_mut() else { return };
    let dir = p.session.target.dir.clone();
    let prev = p.session.data.tracks.get(&dir).cloned().unwrap_or_default();
    let pref = if audio {
        let Some(t) = t else { return };
        p.player.set_audio_track(Some(t.id));
        TrackPref { audio_lang: t.lang, audio_title: t.title, ..prev }
    } else {
        p.player.set_sub_track(t.as_ref().map(|t| t.id));
        TrackPref { sub_off: t.is_none(), sub_lang: t.as_ref().and_then(|t| t.lang.clone()), sub_title: t.and_then(|t| t.title), ..prev }
    };
    p.session.data.tracks.insert(dir.clone(), pref.clone());
    v.state.core.player().save_tracks(dir, pref);
}

pub fn content(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let all = convert::tracks(&p.last);
    let subs: Vec<Track> = all.iter().filter(|t| t.kind == TrackKind::Sub).cloned().collect();
    let audios: Vec<Track> = all.iter().filter(|t| t.kind == TrackKind::Audio).cloned().collect();
    let t = cx.theme().clone();
    let sub_ix = subs.iter().position(|t| t.selected).map_or(0, |i| i + 1);
    let labels = std::iter::once("Desligada".to_owned()).chain(subs.iter().map(track_label));
    let s2 = subs.clone();
    let sub_group = RadioGroup::new("player-legendas", Some(sub_ix))
        .items(labels)
        .kit()
        .on_change(move |v: &mut Higanbana, ix, cx| {
            choose(v, if ix == 0 { None } else { s2.get(ix - 1).cloned() }, false);
            cx.notify();
        })
        .render(cx);
    let mut col = div().w(288.0).flex_col().gap(12.0).child(div().flex_col().child(menu_heading(&t, "Legenda")).child(sub_group));
    if audios.len() > 1 {
        let ix = audios.iter().position(|t| t.selected);
        let labels: Vec<String> = audios.iter().map(track_label).collect();
        let group = RadioGroup::new("player-audios", ix)
            .items(labels)
            .kit()
            .on_change(move |v: &mut Higanbana, ix, cx| {
                choose(v, audios.get(ix).cloned(), true);
                cx.notify();
            })
            .render(cx);
        col = col.child(Separator::horizontal().render(cx)).child(div().flex_col().child(menu_heading(&t, "Áudio")).child(group));
    }
    col
}
