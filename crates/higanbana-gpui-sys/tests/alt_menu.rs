//! Alt sozinho não pode abrir o modo de menu do Windows: o loop modal dele segura a
//! thread da UI (nada redesenha, vídeo parado) até um clique.
use higanbana_gpui_sys::*;
mod common;
use common::Fixture;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(h: *mut c_void, msg: u32, wp: usize, lp: isize) -> isize;
    fn PostMessageW(h: *mut c_void, msg: u32, wp: usize, lp: isize) -> i32;
}

const WM_KEYDOWN: u32 = 0x0100;
const WM_SYSCOMMAND: u32 = 0x0112;
const SC_KEYMENU: usize = 0xF100;
const VK_ESCAPE: usize = 0x1B;

struct Hwnd(*mut c_void);
unsafe impl Send for Hwnd {}

/// Manda `msg` e mede quanto a thread da UI ficou presa. Se o Windows entrar no loop
/// modal, uma thread o desfaz com Esc depois de ~1 s (o teste falha em vez de travar).
fn held(f: &Fixture, msg: u32, wp: usize, lp: isize) -> Duration {
    let h = unsafe { hg__window_hwnd(f.win) };
    let done = Arc::new(AtomicBool::new(false));
    let (d, hw) = (done.clone(), Hwnd(h));
    let guard = std::thread::spawn(move || {
        let hw = hw;
        std::thread::sleep(Duration::from_millis(1000));
        while !d.load(Ordering::Relaxed) {
            unsafe { PostMessageW(hw.0, WM_KEYDOWN, VK_ESCAPE, 0) };
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let t = Instant::now();
    unsafe { SendMessageW(h, msg, wp, lp) };
    // O Windows posta o WM_SYSCOMMAND do Alt: o loop do app é quem o despacha.
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {}
    let took = t.elapsed();
    done.store(true, Ordering::Relaxed);
    guard.join().unwrap();
    took
}

#[test]
fn menu_por_teclado_nao_prende_a_ui() {
    // Soltar o Alt sozinho (ou F10) faz o Windows postar este SC_KEYMENU com lParam 0.
    let f = Fixture::new();
    let took = held(&f, WM_SYSCOMMAND, SC_KEYMENU, 0);
    assert!(took < Duration::from_millis(500), "SC_KEYMENU prendeu a UI por {took:?}");
}
