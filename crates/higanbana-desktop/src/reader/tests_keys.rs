//! Atalhos e cliques do leitor.
use higanbana_core::reader::logic::{Direction, Turn};
use higanbana_gpui::prelude::*;
use higanbana_gpui::Modifiers;

use super::keys::{char_action, escape_step, key_action, side_turn, EscapeStep, ReaderKey};

fn key(k: Key) -> KeyEvent {
    KeyEvent { key: k, mods: Modifiers::default(), repeat: false }
}

#[test]
fn esc_fecha_painel_antes_de_sair() {
    assert_eq!(escape_step(true, true), EscapeStep::ClosePanel);
    assert_eq!(escape_step(false, true), EscapeStep::ExitFullscreen);
    assert_eq!(escape_step(false, false), EscapeStep::Close);
}

#[test]
fn setas_respeitam_direcao() {
    assert_eq!(key_action(&key(Key::Left), false, Direction::Rtl), Some(ReaderKey::Turn(Turn::Next)));
    assert_eq!(key_action(&key(Key::Left), false, Direction::Ltr), Some(ReaderKey::Turn(Turn::Prev)));
    assert_eq!(key_action(&key(Key::Down), true, Direction::Rtl), Some(ReaderKey::Scroll(80.0)));
    assert_eq!(key_action(&key(Key::Down), false, Direction::Rtl), None, "↓ só no long strip");
    assert_eq!(key_action(&key(Key::Char('g')), false, Direction::Rtl), Some(ReaderKey::Index));
    assert_eq!(char_action(']'), Some(ReaderKey::NextChapter));
}

#[test]
fn teclas_ignoradas_com_ctrl() {
    let mut e = key(Key::Char('m'));
    e.mods.ctrl = true;
    assert_eq!(key_action(&e, false, Direction::Rtl), None);
}

#[test]
fn clique_na_lateral_vira_pagina() {
    // Área de 1000 px com margens de 88: x relativo ao miolo.
    assert_eq!(side_turn(100.0, 1000.0, 88.0, Direction::Rtl), Some(Turn::Next));
    assert_eq!(side_turn(100.0, 1000.0, 88.0, Direction::Ltr), Some(Turn::Prev));
    assert_eq!(side_turn(500.0, 1000.0, 88.0, Direction::Ltr), None, "meio não vira");
    assert_eq!(side_turn(900.0, 1000.0, 88.0, Direction::Ltr), Some(Turn::Next));
}
