//! Regras da tela do player que não precisam de janela.

use higanbana_core::player::logic::{Track, TrackKind};
use higanbana_gpui::prelude::*;
use higanbana_gpui::Modifiers;

use super::controls::{slider_value, time_label, volume_icon};
use super::corner::{countdown_fill, next_heading};
use super::keys::{action, escape_step, volume_step, EscapeStep, KeyAction};
use super::menus::settings::thread_label;
use super::menus::tracks::track_label;
use super::seek::{preview_left, THUMB_W};

#[test]
fn icone_volume_por_nivel() {
    assert_eq!(volume_icon(true, 80.0), "volume-x");
    assert_eq!(volume_icon(false, 0.0), "volume-x");
    assert_eq!(volume_icon(false, 49.0), "volume-1");
    assert_eq!(volume_icon(false, 50.0), "volume-2");
}

#[test]
fn slider_zero_quando_mudo() {
    assert_eq!(slider_value(true, 70.0), 0.0);
    assert_eq!(slider_value(false, 70.0), 70.0);
}

#[test]
fn previa_limitada_nas_bordas() {
    assert_eq!(preview_left(10.0, 800.0), 0.0);
    assert_eq!(preview_left(800.0, 800.0), 800.0 - THUMB_W);
    assert_eq!(preview_left(400.0, 800.0), 400.0 - THUMB_W / 2.0);
}

#[test]
fn tempo_formatado() {
    assert_eq!(time_label(65.0, 1425.0), "1:05 / 23:45");
}

#[test]
fn esc_fecha_menu_depois_tela_cheia_depois_player() {
    assert_eq!(escape_step(true, true), EscapeStep::CloseMenu);
    assert_eq!(escape_step(false, true), EscapeStep::ExitFullscreen);
    assert_eq!(escape_step(false, false), EscapeStep::ClosePlayer);
}

#[test]
fn setas_buscam_5s() {
    let k = |key| KeyEvent::new(key, Modifiers::default());
    assert_eq!(action(&k(Key::Left)), Some(KeyAction::Seek(-5.0)));
    assert_eq!(action(&k(Key::Right)), Some(KeyAction::Seek(5.0)));
    assert_eq!(action(&k(Key::Char('n'))), Some(KeyAction::Next));
    assert_eq!(action(&k(Key::Char('s'))), Some(KeyAction::SkipIntro));
}

#[test]
fn ctrl_ignora() {
    let mut m = Modifiers::default();
    m.ctrl = true;
    assert_eq!(action(&KeyEvent::new(Key::Space, m)), None);
}

#[test]
fn volume_sobe_tira_mudo() {
    assert_eq!(volume_step(98.0, true, 5.0), (100.0, true));
    assert_eq!(volume_step(3.0, false, -5.0), (0.0, false));
    assert_eq!(volume_step(0.0, true, -5.0), (0.0, false));
}

#[test]
fn rotulo_da_faixa() {
    let t = Track { id: 2, kind: TrackKind::Sub, lang: Some("por".into()), title: Some("Brasil".into()), selected: false };
    assert!(track_label(&t).ends_with("(POR)"), "{}", track_label(&t));
}

#[test]
fn cartao_proximo() {
    assert_eq!(next_heading(Some(4.2)), "Próximo episódio em 5 s");
    assert_eq!(next_heading(None), "Próximo episódio");
    assert_eq!(countdown_fill(5.0, 5.0), 0.0);
    assert_eq!(countdown_fill(0.0, 5.0), 100.0);
}

#[test]
fn nucleos() {
    assert_eq!(thread_label(0), "Automático");
    assert_eq!(thread_label(1), "1 núcleo");
    assert_eq!(thread_label(4), "4 núcleos");
}

#[test]
fn lente_sem_player_usa_reserva() {
    let mut spare = super::Spare::default();
    let mut none: Option<super::PlayerView> = None;
    super::seek_of(&mut none, &mut spare).drag = Some(0.5);
    assert_eq!(spare.seek.drag, Some(0.5));
}
