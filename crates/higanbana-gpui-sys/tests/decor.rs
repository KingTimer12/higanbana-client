//! Decorations customizadas: área cliente, hit test por regiões, comandos e estado.
use higanbana_gpui_sys::*;
mod common;
use common::{Fixture, OK};
use std::ffi::c_void;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug, PartialEq)]
struct Rect {
    l: i32,
    t: i32,
    r: i32,
    b: i32,
}

#[repr(C)]
#[derive(Default)]
struct MonitorInfo {
    cb: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(h: *mut c_void, msg: u32, wp: usize, lp: isize) -> isize;
    fn GetClientRect(h: *mut c_void, r: *mut Rect) -> i32;
    fn GetWindowRect(h: *mut c_void, r: *mut Rect) -> i32;
    fn ClientToScreen(h: *mut c_void, p: *mut [i32; 2]) -> i32;
    fn MonitorFromWindow(h: *mut c_void, flags: u32) -> *mut c_void;
    fn GetMonitorInfoW(m: *mut c_void, mi: *mut MonitorInfo) -> i32;
}

const WM_NCHITTEST: u32 = 0x0084;
const WM_NCMOUSEMOVE: u32 = 0x00A0;
const HTCLIENT: isize = 1;
const HTCAPTION: isize = 2;
const HTMAXBUTTON: isize = 9;
const HTLEFT: isize = 10;
const HTTOP: isize = 12;
const HTBOTTOMRIGHT: isize = 17;

fn hwnd(f: &Fixture) -> *mut c_void {
    unsafe { hg__window_hwnd(f.win) }
}

fn scale(f: &Fixture) -> f32 {
    unsafe { hg_window_dpi_scale(f.win) }
}

/// Ponto lógico do cliente → lParam com coordenadas de tela.
fn screen_lp(f: &Fixture, x: f32, y: f32) -> isize {
    let s = scale(f);
    let mut p = [(x * s) as i32, (y * s) as i32];
    unsafe { ClientToScreen(hwnd(f), &mut p) };
    ((p[1] as u16 as isize) << 16) | (p[0] as u16 as isize)
}

fn hit(f: &Fixture, x: f32, y: f32) -> isize {
    unsafe { SendMessageW(hwnd(f), WM_NCHITTEST, 0, screen_lp(f, x, y)) }
}

fn region(x: f32, y: f32, w: f32, h: f32, kind: hg_hit) -> hg_hit_region {
    hg_hit_region { x, y, w, h, kind }
}

fn custom(f: &Fixture) {
    assert_eq!(unsafe { hg_window_set_decorations(f.win, 1) }, OK);
    let regions = [
        region(0.0, 0.0, 320.0, 32.0, hg_hit_HG_HIT_CAPTION),
        region(240.0, 0.0, 40.0, 32.0, hg_hit_HG_HIT_MAXIMIZE),
        region(10.0, 4.0, 40.0, 24.0, hg_hit_HG_HIT_CLIENT),
    ];
    assert_eq!(unsafe { hg_window_set_hit_regions(f.win, regions.as_ptr(), 3) }, OK);
}

fn drain(f: &Fixture) -> Vec<hg_event> {
    let mut out = Vec::new();
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {
        out.push(ev);
    }
    out
}

fn client_size(f: &Fixture) -> (i32, i32) {
    let mut r = Rect::default();
    unsafe { GetClientRect(hwnd(f), &mut r) };
    (r.r - r.l, r.b - r.t)
}

#[test]
fn native_hit_test_unchanged() {
    let f = Fixture::new();
    assert_eq!(hit(&f, 160.0, 120.0), HTCLIENT);
    assert_eq!(hit(&f, 160.0, -10.0), HTCAPTION, "barra nativa acima do cliente");
}

#[test]
fn custom_client_covers_window() {
    let f = Fixture::new();
    custom(&f);
    let mut w = Rect::default();
    unsafe { GetWindowRect(hwnd(&f), &mut w) };
    assert_eq!(client_size(&f), (w.r - w.l, w.b - w.t));
}

#[test]
fn regions_map_to_hit_codes() {
    let f = Fixture::new();
    custom(&f);
    let (w, h) = client_size(&f);
    let (w, h) = (w as f32 / scale(&f), h as f32 / scale(&f));
    assert_eq!(hit(&f, 100.0, 16.0), HTCAPTION);
    assert_eq!(hit(&f, 250.0, 16.0), HTMAXBUTTON);
    assert_eq!(hit(&f, 20.0, 16.0), HTCLIENT, "a última região que contém o ponto vence");
    assert_eq!(hit(&f, w / 2.0, h / 2.0), HTCLIENT);
    assert_eq!(hit(&f, 160.0, 2.0), HTTOP);
    assert_eq!(hit(&f, 1.0, h / 2.0), HTLEFT);
    assert_eq!(hit(&f, w - 2.0, h - 2.0), HTBOTTOMRIGHT);
}

#[test]
fn maximize_command_sets_state_and_event() {
    let f = Fixture::new();
    drain(&f);
    assert_eq!(unsafe { hg_window_command(f.win, hg_window_cmd_HG_WINDOW_MAXIMIZE) }, OK);
    assert_ne!(unsafe { hg_window_state(f.win) } & HG_WINDOW_MAXIMIZED, 0);
    assert!(drain(&f).iter().any(|e| e.kind == hg_event_kind_HG_EVENT_WINDOW_STATE));
    assert_eq!(unsafe { hg_window_command(f.win, hg_window_cmd_HG_WINDOW_RESTORE) }, OK);
    assert_eq!(unsafe { hg_window_state(f.win) } & HG_WINDOW_MAXIMIZED, 0);
}

#[test]
fn maximized_client_fits_monitor() {
    let f = Fixture::new();
    custom(&f);
    unsafe { hg_window_command(f.win, hg_window_cmd_HG_WINDOW_MAXIMIZE) };
    let mut c = Rect::default();
    unsafe { GetClientRect(hwnd(&f), &mut c) };
    let mut tl = [c.l, c.t];
    let mut br = [c.r, c.b];
    unsafe { ClientToScreen(hwnd(&f), &mut tl) };
    unsafe { ClientToScreen(hwnd(&f), &mut br) };
    let mut mi = MonitorInfo { cb: size_of::<MonitorInfo>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd(&f), 2), &mut mi) };
    assert_eq!((tl[0], tl[1], br[0], br[1]), (mi.work.l, mi.work.t, mi.work.r, mi.work.b));
}

#[test]
fn max_button_nc_events_become_mouse() {
    let f = Fixture::new();
    custom(&f);
    drain(&f);
    unsafe { SendMessageW(hwnd(&f), WM_NCMOUSEMOVE, HTMAXBUTTON as usize, screen_lp(&f, 250.0, 16.0)) };
    let ev = drain(&f).into_iter().find(|e| e.kind == hg_event_kind_HG_EVENT_MOUSE_MOVE).expect("mouse move");
    let m = unsafe { ev.u.mouse };
    assert!((m.x - 250.0).abs() <= 1.0 && (m.y - 16.0).abs() <= 1.0, "{} {}", m.x, m.y);
}

#[test]
fn fullscreen_roundtrip() {
    let f = Fixture::new();
    let mut before = Rect::default();
    unsafe { GetWindowRect(hwnd(&f), &mut before) };
    assert_eq!(unsafe { hg_window_command(f.win, hg_window_cmd_HG_WINDOW_ENTER_FULLSCREEN) }, OK);
    assert_ne!(unsafe { hg_window_state(f.win) } & HG_WINDOW_FULLSCREEN, 0);
    let mut full = Rect::default();
    unsafe { GetWindowRect(hwnd(&f), &mut full) };
    let mut mi = MonitorInfo { cb: size_of::<MonitorInfo>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd(&f), 2), &mut mi) };
    assert_eq!(full, mi.monitor);
    assert_eq!(unsafe { hg_window_command(f.win, hg_window_cmd_HG_WINDOW_EXIT_FULLSCREEN) }, OK);
    assert_eq!(unsafe { hg_window_state(f.win) } & HG_WINDOW_FULLSCREEN, 0);
    let mut after = Rect::default();
    unsafe { GetWindowRect(hwnd(&f), &mut after) };
    assert_eq!(after, before);
}

#[test]
fn recreate_keeps_custom_frame() {
    let f = Fixture::new();
    custom(&f);
    assert_eq!(unsafe { hg__window_recreate_hwnd(f.win) }, OK);
    assert_eq!(hit(&f, 100.0, 16.0), HTCAPTION);
    let mut w = Rect::default();
    unsafe { GetWindowRect(hwnd(&f), &mut w) };
    assert_eq!(client_size(&f), (w.r - w.l, w.b - w.t));
}
