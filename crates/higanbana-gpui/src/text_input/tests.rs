//! Edição sem GPU: unicode (japonês, emoji), palavras, seleção e área de transferência.
use super::*;
use crate::gfx::Modifiers;
use crate::input::{Key, KeyEvent};

#[derive(Default)]
struct MemClip(Option<String>);

impl Clipboard for MemClip {
    fn set_text(&mut self, t: &str) {
        self.0 = Some(t.into());
    }
    fn get_text(&mut self) -> Option<String> {
        self.0.clone()
    }
}

fn key(k: Key) -> KeyEvent {
    KeyEvent::new(k, Modifiers::default())
}

fn with(k: Key, ctrl: bool, shift: bool) -> KeyEvent {
    KeyEvent::new(k, Modifiers { ctrl, shift, alt: false })
}

fn typed(s: &str) -> TextInputState {
    let mut t = TextInputState::new();
    for c in s.chars() {
        t.handle_char(c);
    }
    t
}

#[test]
fn typing_japanese_and_emoji_keeps_boundaries() {
    let mut t = typed("日本😀a");
    assert_eq!(t.text(), "日本😀a");
    let mut clip = MemClip::default();
    let mut stops = vec![t.caret()];
    for _ in 0..5 {
        t.handle_key(&key(Key::Left), &mut clip);
        assert!(t.text().is_char_boundary(t.caret()));
        stops.push(t.caret());
    }
    assert_eq!(stops, vec![11, 10, 6, 3, 0, 0]);
    t.handle_key(&key(Key::Right), &mut clip);
    t.handle_key(&key(Key::Right), &mut clip);
    t.handle_key(&key(Key::Backspace), &mut clip);
    assert_eq!(t.text(), "日😀a");
    t.handle_key(&key(Key::Delete), &mut clip);
    assert_eq!(t.text(), "日a", "delete remove o emoji inteiro");
}

#[test]
fn zwj_emoji_is_one_grapheme() {
    let family = "👨‍👩‍👧";
    let mut t = TextInputState::with_text(format!("x{family}y"));
    t.move_end(false);
    t.move_left(false, false);
    t.move_left(false, false);
    assert_eq!(t.caret(), 1, "pula a família inteira");
    t.delete(false);
    assert_eq!(t.text(), "xy");
}

#[test]
fn word_movement_and_deletion() {
    let mut t = TextInputState::with_text("olá mundo, tudo bem");
    let mut clip = MemClip::default();
    t.handle_key(&with(Key::Left, true, false), &mut clip);
    assert_eq!(&t.text()[t.caret()..], "bem");
    t.handle_key(&with(Key::Left, true, false), &mut clip);
    assert_eq!(&t.text()[t.caret()..], "tudo bem");
    t.handle_key(&with(Key::Home, false, false), &mut clip);
    t.handle_key(&with(Key::Right, true, false), &mut clip);
    assert_eq!(&t.text()[t.caret()..], "mundo, tudo bem");
    t.handle_key(&with(Key::End, false, false), &mut clip);
    t.handle_key(&with(Key::Backspace, true, false), &mut clip);
    assert_eq!(t.text(), "olá mundo, tudo ");
}

#[test]
fn selection_replace_and_shift_movement() {
    let mut t = TextInputState::with_text("abcdef");
    let mut clip = MemClip::default();
    t.handle_key(&with(Key::Left, false, true), &mut clip);
    t.handle_key(&with(Key::Left, false, true), &mut clip);
    assert_eq!(t.selected_text(), "ef");
    t.handle_char('Z');
    assert_eq!(t.text(), "abcdZ");
    t.handle_key(&with(Key::Home, false, true), &mut clip);
    assert_eq!(t.selection(), 0..5);
    t.handle_key(&key(Key::Right), &mut clip);
    assert_eq!((t.caret(), t.has_selection()), (5, false), "seta sem shift recolhe a seleção");
}

#[test]
fn clipboard_shortcuts() {
    let mut t = TextInputState::with_text("copiar 日本");
    let mut clip = MemClip::default();
    assert_eq!(t.handle_key(&with(Key::Char('a'), true, false), &mut clip), InputOutcome::Moved);
    assert_eq!(t.handle_key(&with(Key::Char('c'), true, false), &mut clip), InputOutcome::Moved);
    assert_eq!(clip.0.as_deref(), Some("copiar 日本"));
    assert_eq!(t.handle_key(&with(Key::Char('x'), true, false), &mut clip), InputOutcome::Edited);
    assert_eq!(t.text(), "");
    clip.0 = Some("a\r\nb".into());
    t.handle_key(&with(Key::Char('v'), true, false), &mut clip);
    assert_eq!(t.text(), "a b", "colar em uma linha só");
}

#[test]
fn ignores_keys_that_should_bubble_and_controls() {
    let mut t = TextInputState::new();
    let mut clip = MemClip::default();
    for k in [Key::Tab, Key::Enter, Key::Escape] {
        assert_eq!(t.handle_key(&key(k), &mut clip), InputOutcome::Ignored);
    }
    assert_eq!(t.handle_char('\u{8}'), InputOutcome::Ignored);
    assert!(!t.backspace(false), "vazio: nada a apagar");
}

#[test]
fn set_caret_snaps_to_char_boundary() {
    let mut t = TextInputState::with_text("日本");
    t.set_caret(4, false);
    assert_eq!(t.caret(), 3);
    t.select_word_at(1);
    assert_eq!(t.selected_text(), "日本");
}
