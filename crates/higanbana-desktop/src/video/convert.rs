//! Estado da lib → tipos da lógica do núcleo (capítulos e faixas).

use higanbana_core::player::logic::{Chapter, Track, TrackKind};
use higanbana_gpui::video::PlayerState;

pub fn chapters(st: &PlayerState) -> Vec<Chapter> {
    st.chapters.iter().map(|c| Chapter { title: c.title.clone(), time: c.time }).collect()
}

fn opt(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

/// Áudio e legendas, com `selected` vindo de `aid`/`sid`.
pub fn tracks(st: &PlayerState) -> Vec<Track> {
    let one = |t: &higanbana_gpui::video::Track, kind, cur: Option<i64>| Track { id: t.id, kind, lang: opt(&t.lang), title: opt(&t.title), selected: cur == Some(t.id) };
    let audio = st.audio_tracks.iter().map(|t| one(t, TrackKind::Audio, st.audio));
    let subs = st.sub_tracks.iter().map(|t| one(t, TrackKind::Sub, st.sub));
    audio.chain(subs).collect()
}
