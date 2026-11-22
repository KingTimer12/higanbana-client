//! Botões laterais do mouse (XBUTTON1/2) viram eventos com botão 3/4.
use higanbana_gpui_sys::*;
mod common;
use common::Fixture;
use std::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(h: *mut c_void, msg: u32, wp: usize, lp: isize) -> isize;
}

const WM_XBUTTONUP: u32 = 0x020C;
const XBUTTON1: usize = 1;
const XBUTTON2: usize = 2;

fn ups(f: &Fixture) -> Vec<u32> {
    let mut out = Vec::new();
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {
        if ev.kind == hg_event_kind_HG_EVENT_MOUSE_UP {
            out.push(unsafe { ev.u.mouse.button });
        }
    }
    out
}

#[test]
fn xbuttons_viram_botoes_3_e_4() {
    let f = Fixture::new();
    let h = unsafe { hg__window_hwnd(f.win) };
    ups(&f);
    // HIWORD(wParam) = qual botão; lParam = posição no cliente (10, 10).
    let lp = (10 << 16) | 10;
    let r = unsafe { SendMessageW(h, WM_XBUTTONUP, XBUTTON1 << 16, lp) };
    assert_eq!(r, 1, "WM_XBUTTONUP deve devolver TRUE");
    unsafe { SendMessageW(h, WM_XBUTTONUP, XBUTTON2 << 16, lp) };
    assert_eq!(ups(&f), vec![3, 4]);
}
