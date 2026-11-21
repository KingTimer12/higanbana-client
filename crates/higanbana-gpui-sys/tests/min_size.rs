//! Tamanho mínimo da janela (WM_GETMINMAXINFO) em pixels lógicos.
use higanbana_gpui_sys::*;
mod common;
use common::{Fixture, OK};
use std::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(h: *mut c_void, msg: u32, wp: usize, lp: isize) -> isize;
}

const WM_GETMINMAXINFO: u32 = 0x0024;

#[repr(C)]
#[derive(Default)]
struct MinMaxInfo {
    reserved: [i32; 2],
    max_size: [i32; 2],
    max_position: [i32; 2],
    min_track: [i32; 2],
    max_track: [i32; 2],
}

fn min_track(f: &Fixture) -> [i32; 2] {
    let mut mmi = MinMaxInfo::default();
    unsafe { SendMessageW(hg__window_hwnd(f.win), WM_GETMINMAXINFO, 0, &mut mmi as *mut _ as isize) };
    mmi.min_track
}

#[test]
fn minimo_vira_pixels_fisicos_da_janela() {
    let f = Fixture::new();
    assert_eq!(unsafe { hg_window_set_decorations(f.win, 1) }, OK);
    assert_eq!(unsafe { hg_window_set_min_size(f.win, 960.0, 600.0) }, OK);
    let s = unsafe { hg_window_dpi_scale(f.win) };
    // Barra própria: janela = área cliente.
    assert_eq!(min_track(&f), [(960.0 * s).round() as i32, (600.0 * s).round() as i32]);
}

#[test]
fn argumento_invalido_falha() {
    let f = Fixture::new();
    assert_ne!(unsafe { hg_window_set_min_size(f.win, -1.0, 10.0) }, OK);
}
