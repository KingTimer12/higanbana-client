//! Imagens no backend WGPU (espelha `image.rs`, que roda no D3D11).
#![cfg(feature = "wgpu")]
use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;
mod common;
use common::OK;

const WGPU: hg_backend_kind = hg_backend_kind_HG_BACKEND_WGPU;
const D3D11: hg_backend_kind = hg_backend_kind_HG_BACKEND_D3D11;
const WHITE: hg_color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

fn wgpu_available() -> bool {
    assert!(wgpu_backend::install());
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n].iter().any(|i| i.kind == WGPU)
}

fn last_error() -> String {
    unsafe { CStr::from_ptr(hg_last_error()) }.to_string_lossy().into_owned()
}

struct Fixture { ctx: *mut hg_context, win: *mut hg_window, dl: *mut hg_draw_list }

impl Fixture {
    fn new(kind: hg_backend_kind) -> Self {
        unsafe {
            let mut ctx = ptr::null_mut();
            assert_eq!(hg_context_create(kind, &mut ctx), OK, "{}", last_error());
            let title = CString::new("wgpu imagens").unwrap();
            let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 320, height: 240, resizable: 1 };
            let mut win = ptr::null_mut();
            assert_eq!(hg_window_create(ctx, &desc, &mut win), OK, "{}", last_error());
            Fixture { ctx, win, dl: hg_draw_list_create() }
        }
    }

    fn pixel(&self, x: u32, y: u32) -> u32 {
        let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let mut px = 0;
        unsafe {
            assert_eq!(hg_frame_begin(self.win, clear), OK, "{}", last_error());
            assert_eq!(hg_frame_submit(self.win, self.dl), OK, "{}", last_error());
            assert_eq!(hg__window_read_pixel(self.win, x, y, &mut px), OK, "{}", last_error());
            assert_eq!(hg_frame_end(self.win, 0), OK, "{}", last_error());
        }
        px & 0x00ff_ffff
    }

    fn stats(&self) -> hg_stats {
        let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
        unsafe { hg_context_stats(self.ctx, &mut s) };
        s
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe {
            hg_draw_list_destroy(self.dl);
            hg_window_destroy(self.win);
            hg_context_destroy(self.ctx);
        }
    }
}

fn solid_image(ctx: *mut hg_context, rgba: [u8; 4], w: u32, h: u32) -> *mut hg_image {
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
    let mut img = ptr::null_mut();
    unsafe { assert_eq!(hg_image_create(ctx, w, h, px.as_ptr(), w * 4, &mut img), OK) };
    img
}

fn draw(rect: hg_rect, tint: hg_color, radius: f32) -> hg_image_draw {
    hg_image_draw { rect, uv: hg_rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }, tint, radius: [radius; 4] }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> hg_rect {
    hg_rect { x, y, w, h }
}

#[test]
fn image_pixels_reach_screen_with_tint() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    let img = solid_image(f.ctx, [0, 255, 0, 255], 4, 4);
    let half = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 0.5 };
    unsafe {
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 0.0, 40.0, 40.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(50.0, 0.0, 40.0, 40.0), half, 0.0)), OK);
    }
    assert_eq!(f.pixel(20, 20), 0x0000_ff00);
    let g = (f.pixel(70, 20) >> 8) & 0xff;
    assert!((120..=136).contains(&g), "meio alfa: {g}");
    let s = f.stats();
    assert_eq!(s.draw_calls, 1, "mesma imagem vizinha = um draw");
    assert!(s.gpu_bytes >= 4 * 4 * 4, "textura conta em gpu_bytes");
    unsafe { hg_image_destroy(img) };
}

#[test]
fn different_images_alternating_split_draws() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    let a = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    let b = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    unsafe {
        for (i, img) in [a, a, b, a].into_iter().enumerate() {
            let r = rect(i as f32 * 50.0, 0.0, 40.0, 40.0);
            assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
        }
    }
    assert_eq!(f.pixel(120, 20), 0x0000_00ff);
    assert_eq!(f.pixel(170, 20), 0x00ff_0000);
    assert_eq!(f.stats().draw_calls, 3, "a,a | b | a");
    unsafe { hg_image_destroy(a); hg_image_destroy(b) };
}

#[test]
fn rounded_image_corner_is_background() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 0.0, 60.0, 60.0), WHITE, 30.0)), OK) };
    assert_eq!(f.pixel(1, 1), 0);
    assert_eq!(f.pixel(30, 30), 0x00ff_0000);
    unsafe { hg_image_destroy(img) };
}

#[test]
fn layer_puts_quad_over_image() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    let r = rect(0.0, 0.0, 60.0, 60.0);
    let q = hg_quad { rect: r, fill: hg_color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }, ..unsafe { std::mem::zeroed() } };
    unsafe {
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
    }
    assert_eq!(f.pixel(30, 30), 0x00ff_0000, "imagem sobre quad na mesma camada");
    unsafe {
        assert_eq!(hg_draw_list_layer(f.dl), OK);
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
    }
    assert_eq!(f.pixel(30, 30), 0x0000_00ff, "camada nova fica por cima");
    unsafe { hg_image_destroy(img) };
}

#[test]
fn switch_d3d11_wgpu_d3d11_keeps_image() {
    if !wgpu_available() { return }
    let f = Fixture::new(D3D11);
    let img = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    let _leak = solid_image(f.ctx, [1, 2, 3, 255], 2, 2); // liberada por hg_context_destroy
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 0.0, 40.0, 40.0), WHITE, 0.0)), OK) };
    for kind in [D3D11, WGPU, D3D11, WGPU] {
        assert_eq!(unsafe { hg_context_switch_backend(f.ctx, kind) }, OK, "{}", last_error());
        assert_eq!(f.pixel(20, 20), 0x0000_00ff, "{kind}: textura recriada após troca");
    }
    unsafe { hg_image_destroy(img) };
}
