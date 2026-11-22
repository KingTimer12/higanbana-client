//! Imagens no backend D3D12 (pulado se o D3D12 não estiver disponível).
mod common;
use common::OK;
use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;

const D3D11: hg_backend_kind = hg_backend_kind_HG_BACKEND_D3D11;
const D3D12: hg_backend_kind = hg_backend_kind_HG_BACKEND_D3D12;
const WHITE: hg_color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

fn d3d12_available() -> bool {
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n].iter().any(|i| i.kind == D3D12)
}

struct Fx {
    ctx: *mut hg_context,
    win: *mut hg_window,
    dl: *mut hg_draw_list,
}

impl Fx {
    fn new(kind: hg_backend_kind) -> Self {
        unsafe {
            let mut ctx = ptr::null_mut();
            assert_eq!(hg_context_create(kind, &mut ctx), OK, "{:?}", CStr::from_ptr(hg_last_error()));
            let title = CString::new("image_d3d12").unwrap();
            let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 320, height: 240, resizable: 1 };
            let mut win = ptr::null_mut();
            assert_eq!(hg_window_create(ctx, &desc, &mut win), OK, "{:?}", CStr::from_ptr(hg_last_error()));
            Fx { ctx, win, dl: hg_draw_list_create() }
        }
    }

    fn pixel(&self, x: u32, y: u32) -> u32 {
        let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let mut px = 0;
        unsafe {
            assert_eq!(hg_frame_begin(self.win, clear), OK);
            assert_eq!(hg_frame_submit(self.win, self.dl), OK, "{:?}", CStr::from_ptr(hg_last_error()));
            assert_eq!(hg__window_read_pixel(self.win, x, y, &mut px), OK);
            assert_eq!(hg_frame_end(self.win, 0), OK);
        }
        px & 0x00ff_ffff
    }

    fn image(&self, rgba: [u8; 4], w: u32, h: u32) -> *mut hg_image {
        let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
        let mut img = ptr::null_mut();
        unsafe { assert_eq!(hg_image_create(self.ctx, w, h, px.as_ptr(), w * 4, &mut img), OK) };
        img
    }
}

impl Drop for Fx {
    fn drop(&mut self) {
        unsafe {
            hg_draw_list_destroy(self.dl);
            hg_window_destroy(self.win);
            hg_context_destroy(self.ctx);
        }
    }
}

fn draw(rect: hg_rect, tint: hg_color, radius: f32) -> hg_image_draw {
    hg_image_draw { rect, uv: hg_rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }, tint, radius: [radius; 4] }
}

fn rect(x: f32, w: f32) -> hg_rect {
    hg_rect { x, y: 0.0, w, h: w }
}

#[test]
fn image_pixels_reach_screen_with_tint() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    let img = f.image([0, 255, 0, 255], 4, 4);
    unsafe {
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 40.0), WHITE, 0.0)), OK);
        let half = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 0.5 };
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(50.0, 40.0), half, 0.0)), OK);
    }
    for _ in 0..3 {
        assert_eq!(f.pixel(20, 20), 0x0000_ff00);
        let g = (f.pixel(70, 20) >> 8) & 0xff;
        assert!((120..=136).contains(&g), "meio alfa: {g}");
    }
    unsafe { hg_image_destroy(img) };
}

#[test]
fn draw_calls_group_neighbour_images() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    let a = f.image([255, 0, 0, 255], 2, 2);
    let b = f.image([0, 0, 255, 255], 2, 2);
    unsafe {
        for (i, img) in [a, a, b, b, a].iter().enumerate() {
            assert_eq!(hg_draw_image(f.dl, *img, &draw(rect(i as f32 * 30.0, 20.0), WHITE, 0.0)), OK);
        }
    }
    assert_eq!(f.pixel(10, 10), 0x00ff_0000);
    assert_eq!(f.pixel(70, 10), 0x0000_00ff);
    let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    assert_eq!(s.draw_calls, 3, "a,a | b,b | a");
    assert!(s.gpu_bytes >= 2 * 2 * 4 * 2, "texturas contam em gpu_bytes");
    unsafe {
        hg_image_destroy(a);
        hg_image_destroy(b);
    }
}

#[test]
fn rounded_image_corner_is_background() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    let img = f.image([255, 0, 0, 255], 2, 2);
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 60.0), WHITE, 30.0)), OK) };
    assert_eq!(f.pixel(1, 1), 0);
    assert_eq!(f.pixel(30, 30), 0x00ff_0000);
    unsafe { hg_image_destroy(img) };
}

#[test]
fn layer_puts_quad_over_image() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    let img = f.image([255, 0, 0, 255], 2, 2);
    unsafe {
        let r = rect(0.0, 60.0);
        let q = hg_quad { rect: r, fill: hg_color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }, ..std::mem::zeroed() };
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
        assert_eq!(f.pixel(30, 30), 0x00ff_0000, "imagem sobre quad na mesma camada");
        assert_eq!(hg_draw_list_layer(f.dl), OK);
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(f.pixel(30, 30), 0x0000_00ff, "camada nova fica por cima");
        hg_image_destroy(img);
    }
}

#[test]
fn destroy_while_frames_in_flight_then_recreate() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    for i in 0..20u8 {
        let img = f.image([i * 10, 255, 0, 255], 8, 8);
        unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 40.0), WHITE, 0.0)), OK) };
        assert_eq!((f.pixel(20, 20) >> 8) & 0xff, 0xff);
        unsafe {
            hg_image_destroy(img);
            hg_draw_list_reset(f.dl);
        }
    }
}

#[test]
fn switch_d3d11_d3d12_d3d11_redraws_image() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D11);
    let img = f.image([0, 0, 255, 255], 2, 2);
    // Imagem com update (dinâmica) guarda a cópia: é ela que a troca recria.
    let px: Vec<u8> = (0..4).flat_map(|_| [0u8, 0, 255, 255]).collect();
    unsafe { assert_eq!(hg_image_update(img, px.as_ptr(), 8), OK) };
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 40.0), WHITE, 0.0)), OK) };
    assert_eq!(f.pixel(20, 20), 0x0000_00ff);
    for kind in [D3D12, D3D11, D3D12, D3D11] {
        unsafe {
            assert_eq!(hg_context_switch_backend(f.ctx, kind), OK, "{:?}", CStr::from_ptr(hg_last_error()));
        }
        assert_eq!(f.pixel(20, 20), 0x0000_00ff, "{kind:?}");
    }
    unsafe { hg_image_destroy(img) };
}
