//! Menu "Ajustes": próximo sozinho, contagem, pular abertura, idiomas, miniaturas,
//! processador e buffer. Grava na hora (e aplica no mpv o que vale ao vivo).

use higanbana_core::player::store::Settings;
use higanbana_gpui::components::{Component, RadioGroup, Separator};
use higanbana_gpui::prelude::*;

use super::super::style::menu_heading;
use super::switch_row;
use crate::root::Higanbana;
use crate::video::CACHES;

pub const LANGS: [(&str, &str); 5] = [("por", "Português"), ("eng", "Inglês"), ("jpn", "Japonês"), ("kor", "Coreano"), ("spa", "Espanhol")];
pub const COUNTDOWNS: [u8; 3] = [5, 10, 15];
/// Núcleos do decodificador: 0 = o mpv escolhe.
pub const THREADS: [u8; 4] = [0, 4, 2, 1];

pub fn thread_label(n: u8) -> String {
    match n {
        0 => "Automático".into(),
        1 => "1 núcleo".into(),
        n => format!("{n} núcleos"),
    }
}

/// Muda e grava; `demuxer-max-bytes` e `vd-lavc-threads` vão ao mpv já.
pub fn change(v: &mut Higanbana, f: impl FnOnce(&mut Settings)) {
    let Some(p) = v.player.as_mut() else { return };
    let mut s = p.session.data.settings.clone();
    f(&mut s);
    let old = &p.session.data.settings;
    if s.cache_mib != old.cache_mib {
        p.player.set_property("demuxer-max-bytes", &format!("{}MiB", s.cache_mib));
    }
    if s.back_cache_mib != old.back_cache_mib {
        p.player.set_property("demuxer-max-back-bytes", &format!("{}MiB", s.back_cache_mib));
    }
    if s.decode_threads != old.decode_threads {
        p.player.set_property("vd-lavc-threads", &s.decode_threads.to_string());
    }
    p.session.data.settings = s.clone();
    v.state.core.player().save_settings(s);
}

fn group(t: &Theme, label: &'static str, radios: Element<Higanbana>) -> Element<Higanbana> {
    div().flex_col().gap(4.0).child(menu_heading(t, label)).child(radios)
}

fn radios(id: &'static str, labels: Vec<String>, selected: Option<usize>, cx: &mut RenderCx<'_, Higanbana>, f: impl Fn(&mut Higanbana, usize) + 'static) -> Element<Higanbana> {
    RadioGroup::new(id, selected)
        .items(labels)
        .horizontal()
        .on_change(move |v: &mut Higanbana, ix, cx| {
            f(v, ix);
            cx.notify();
        })
        .render(cx)
}

pub fn content(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let s = p.session.settings().clone();
    let t = cx.theme().clone();
    let lang = |code: &str| LANGS.iter().position(|(c, _)| *c == code);
    let lang_names = || LANGS.iter().map(|(_, n)| n.to_string()).collect::<Vec<_>>();
    let auto = switch_row("player-auto-proximo", "Ir para o próximo episódio sozinho", s.auto_next, cx, |v, b| change(v, |s| s.auto_next = b));
    let skip = switch_row("player-mostrar-pular", "Mostrar “Pular abertura”", s.show_skip_intro, cx, |v, b| change(v, |s| s.show_skip_intro = b));
    let thumbs = switch_row("player-miniaturas", "Miniatura ao passar o mouse na barra", s.thumbnails, cx, |v, b| change(v, |s| s.thumbnails = b));
    let sub = radios("player-legenda-pref", lang_names(), lang(&s.sub_lang), cx, |v, ix| change(v, |s| s.sub_lang = LANGS[ix].0.into()));
    let audio = radios("player-audio-pref", lang_names(), lang(&s.audio_lang), cx, |v, ix| change(v, |s| s.audio_lang = LANGS[ix].0.into()));
    let threads = radios("player-nucleos", THREADS.map(thread_label).to_vec(), THREADS.iter().position(|n| *n == s.decode_threads), cx, |v, ix| {
        change(v, |s| s.decode_threads = THREADS[ix])
    });
    let cache = radios("player-buffer", CACHES.map(|n| format!("{n} MB")).to_vec(), CACHES.iter().position(|n| *n == s.cache_mib), cx, |v, ix| change(v, |s| s.cache_mib = CACHES[ix]));
    let mut col = div().w(320.0).flex_col().gap(12.0).child(auto);
    if s.auto_next {
        let count = radios("player-contagem", COUNTDOWNS.map(|n| format!("{n} s")).to_vec(), COUNTDOWNS.iter().position(|n| *n == s.countdown_sec), cx, |v, ix| {
            change(v, |s| s.countdown_sec = COUNTDOWNS[ix])
        });
        col = col.child(group(&t, "Tempo da contagem", count));
    }
    col.child(skip)
        .child(Separator::horizontal().render(cx))
        .child(group(&t, "Legenda preferida", sub))
        .child(group(&t, "Áudio preferido", audio))
        .child(Separator::horizontal().render(cx))
        .child(thumbs)
        .child(group(&t, "Processador (vale no próximo episódio)", threads))
        .child(group(&t, "Buffer à frente", cache))
}
