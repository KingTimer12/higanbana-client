//! hg_image_update em todos os backends disponíveis e hg_window_wake entre threads.
use higanbana_gpui_sys::*;
mod common;
use common::{Fixture, OK};
use std::ptr;

const WHITE: hg_color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

fn solid(rgba: [u8; 4], w: u32, h: u32) -> Vec<u8> {
    (0..w * h).flat_map(|_| rgba).collect()
}

fn draw_and_read(f: &Fixture, img: *mut hg_image) -> u32 {
    let d = hg_image_draw {
        rect: hg_rect { x: 0.0, y: 0.0, w: 40.0, h: 40.0 },
        uv: hg_rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
        tint: WHITE,
        radius: [0.0; 4],
    };
    let mut px = 0;
    unsafe {
        hg_draw_list_reset(f.dl);
        assert_eq!(hg_draw_image(f.dl, img, &d), OK);
        assert_eq!(hg_frame_begin(f.win, hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), OK);
        assert_eq!(hg_frame_submit(f.win, f.dl), OK, "{:?}", std::ffi::CStr::from_ptr(hg_last_error()));
        assert_eq!(hg__window_read_pixel(f.win, 20, 20, &mut px), OK);
        assert_eq!(hg_frame_end(f.win, 0), OK);
    }
    px & 0x00ff_ffff
}

/// Backends nativos disponíveis (WGPU tem o próprio teste: exige install antes do enumerate).
fn native_backends() -> Vec<hg_backend_kind> {
    let mut infos: [hg_backend_info; 8] = unsafe { std::mem::zeroed() };
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n.min(8)].iter().map(|i| i.kind).filter(|k| *k != hg_backend_kind_HG_BACKEND_WGPU).collect()
}

const RED: u32 = 0x00ff_0000;
const BLUE: u32 = 0x0000_00ff;

#[test]
fn update_changes_drawn_pixels() {
    let f = Fixture::new();
    for kind in native_backends() {
        assert_eq!(unsafe { hg_context_switch_backend(f.ctx, kind) }, OK, "troca para {kind}");
        let mut img = ptr::null_mut();
        unsafe { assert_eq!(hg_image_create(f.ctx, 4, 4, solid([255, 0, 0, 255], 4, 4).as_ptr(), 16, &mut img), OK) };
        assert_eq!(draw_and_read(&f, img), RED, "backend {kind} antes");
        unsafe { assert_eq!(hg_image_update(img, solid([0, 0, 255, 255], 4, 4).as_ptr(), 16), OK) };
        assert_eq!(draw_and_read(&f, img), BLUE, "backend {kind} depois do update");
        unsafe { hg_image_destroy(img) };
    }
}

#[test]
fn update_wrong_args() {
    let f = Fixture::new();
    let mut img = ptr::null_mut();
    let px = solid([1, 2, 3, 255], 4, 4);
    unsafe {
        assert_eq!(hg_image_create(f.ctx, 4, 4, px.as_ptr(), 16, &mut img), OK);
        assert_eq!(hg_image_update(img, px.as_ptr(), 8), HG_ERR_INVALID as i32);
        assert_eq!(hg_image_update(img, ptr::null(), 16), HG_ERR_INVALID as i32);
        hg_image_destroy(img);
    }
}

#[test]
fn update_survives_backend_switch() {
    let f = Fixture::new();
    let mut img = ptr::null_mut();
    unsafe { assert_eq!(hg_image_create(f.ctx, 4, 4, solid([255, 0, 0, 255], 4, 4).as_ptr(), 16, &mut img), OK) };
    assert_eq!(draw_and_read(&f, img), RED);
    unsafe { assert_eq!(hg_image_update(img, solid([0, 0, 255, 255], 4, 4).as_ptr(), 16), OK) };
    if native_backends().contains(&hg_backend_kind_HG_BACKEND_OPENGL) {
        assert_eq!(unsafe { hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_OPENGL) }, OK);
    }
    assert_eq!(draw_and_read(&f, img), BLUE);
    unsafe { hg_image_destroy(img) };
}

struct SendWin(*mut hg_window);
unsafe impl Send for SendWin {}

#[test]
fn wake_from_other_thread() {
    let f = Fixture::new();
    let mut ev: hg_event = unsafe { std::mem::zeroed() };
    while unsafe { hg_window_poll_event(f.win, &mut ev) } != 0 {}
    let w = SendWin(f.win);
    let t = std::thread::spawn(move || {
        let w = w;
        std::thread::sleep(std::time::Duration::from_millis(50));
        unsafe { hg_window_wake(w.0) };
    });
    let mut got = false;
    for _ in 0..20 {
        assert_eq!(unsafe { hg_window_wait_event_timeout(f.win, 500, &mut ev) }, OK);
        if ev.kind == hg_event_kind_HG_EVENT_WAKE {
            got = true;
            break;
        }
    }
    t.join().unwrap();
    assert!(got, "HG_EVENT_WAKE não chegou");
}
