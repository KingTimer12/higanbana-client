//! Área de transferência, espera com prazo e cursor.
use higanbana_gpui_sys::*;
use std::time::Instant;
mod common;
use common::{Fixture, OK};

fn get_clipboard() -> String {
    let mut len = 0usize;
    assert_eq!(unsafe { hg_clipboard_get_text(std::ptr::null_mut(), 0, &mut len) }, OK);
    let mut buf = vec![0u8; len];
    assert_eq!(unsafe { hg_clipboard_get_text(buf.as_mut_ptr().cast(), buf.len(), &mut len) }, OK);
    String::from_utf8(buf).unwrap()
}

#[test]
fn clipboard_round_trip_unicode() {
    let text = "copiar 日本語 😀 ok";
    assert_eq!(unsafe { hg_clipboard_set_text(text.as_ptr().cast(), text.len()) }, OK);
    assert_eq!(get_clipboard(), text);
    let bad = [0xffu8];
    assert_eq!(unsafe { hg_clipboard_set_text(bad.as_ptr().cast(), 1) }, HG_ERR_INVALID as i32);
}

#[test]
fn wait_timeout_returns_none_after_deadline() {
    let f = Fixture::new();
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {}
    let t = Instant::now();
    assert_eq!(unsafe { hg_window_wait_event_timeout(f.win, 60, &mut ev) }, OK);
    assert_eq!(ev.kind, hg_event_kind_HG_EVENT_NONE);
    assert!(t.elapsed().as_millis() >= 45, "{:?}", t.elapsed());
}

#[test]
fn wait_timeout_returns_queued_event_immediately() {
    let f = Fixture::new();
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {}
    unsafe { hg__window_inject_resize(f.win, 100, 100) };
    let t = Instant::now();
    assert_eq!(unsafe { hg_window_wait_event_timeout(f.win, 5000, &mut ev) }, OK);
    assert_eq!(ev.kind, hg_event_kind_HG_EVENT_RESIZE);
    assert!(t.elapsed().as_millis() < 1000);
}

#[test]
fn cursor_set_and_get() {
    let f = Fixture::new();
    assert_eq!(unsafe { hg_window_cursor(f.win) }, hg_cursor_HG_CURSOR_ARROW);
    assert_eq!(unsafe { hg_window_set_cursor(f.win, hg_cursor_HG_CURSOR_IBEAM) }, OK);
    assert_eq!(unsafe { hg_window_cursor(f.win) }, hg_cursor_HG_CURSOR_IBEAM);
    assert_eq!(unsafe { hg_window_set_cursor(f.win, 99) }, HG_ERR_INVALID as i32);
}
