//! Atalhos e cliques do leitor (puros; quem aplica é `view`).

use higanbana_core::reader::logic::{self, Direction, Turn};
use higanbana_gpui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReaderKey {
    Escape,
    Index,
    Direction,
    Mode,
    Fullscreen,
    PrevChapter,
    NextChapter,
    /// Long strip: rolar em px (positivo = para baixo).
    Scroll(f32),
    Turn(Turn),
}

/// Tecla → ação. Ignora com Ctrl/Alt.
pub fn key_action(ev: &KeyEvent, long_strip: bool, dir: Direction) -> Option<ReaderKey> {
    if ev.mods.ctrl || ev.mods.alt {
        return None;
    }
    let name = match ev.key {
        Key::Escape => return Some(ReaderKey::Escape),
        Key::Char('g' | 'G') => return Some(ReaderKey::Index),
        Key::Char('d' | 'D') => return Some(ReaderKey::Direction),
        Key::Char('m' | 'M') => return Some(ReaderKey::Mode),
        Key::Char('f' | 'F') => return Some(ReaderKey::Fullscreen),
        Key::Up if long_strip => return Some(ReaderKey::Scroll(-80.0)),
        Key::Down if long_strip => return Some(ReaderKey::Scroll(80.0)),
        Key::Space => "space",
        Key::PageDown => "pagedown",
        Key::PageUp => "pageup",
        Key::Left => "left",
        Key::Right => "right",
        _ => return None,
    };
    logic::key_action(name, dir).map(ReaderKey::Turn)
}

/// `[` e `]` chegam como caractere (dependem do layout do teclado).
pub fn char_action(c: char) -> Option<ReaderKey> {
    match c {
        '[' => Some(ReaderKey::PrevChapter),
        ']' => Some(ReaderKey::NextChapter),
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
pub enum EscapeStep {
    ClosePanel,
    ExitFullscreen,
    Close,
}

pub fn escape_step(panel_open: bool, fullscreen: bool) -> EscapeStep {
    if panel_open {
        EscapeStep::ClosePanel
    } else if fullscreen {
        EscapeStep::ExitFullscreen
    } else {
        EscapeStep::Close
    }
}

/// Clique na área da página (x na janela): laterais viram a página respeitando a direção.
pub fn side_turn(x: f32, window_w: f32, margin: f32, dir: Direction) -> Option<Turn> {
    let f = (x - margin) / (window_w - margin * 2.0).max(1.0);
    let rtl = dir == Direction::Rtl;
    if f < 0.4 {
        Some(if rtl { Turn::Next } else { Turn::Prev })
    } else if f > 0.6 {
        Some(if rtl { Turn::Prev } else { Turn::Next })
    } else {
        None
    }
}
