#![cfg(feature = "wgpu")]
use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;
mod common;
use common::OK;

const WGPU: hg_backend_kind = hg_backend_kind_HG_BACKEND_WGPU;

/// Registra o backend e diz se o WGPU foi enumerado nesta máquina.
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
            let title = CString::new("wgpu").unwrap();
            let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 320, height: 240, resizable: 1 };
            let mut win = ptr::null_mut();
            assert_eq!(hg_window_create(ctx, &desc, &mut win), OK, "{}", last_error());
            Fixture { ctx, win, dl: hg_draw_list_create() }
        }
    }

    fn quad(&self, x: f32, y: f32, w: f32, h: f32, fill: hg_color) {
        let q = hg_quad { rect: hg_rect { x, y, w, h }, fill, ..unsafe { std::mem::zeroed() } };
        assert_eq!(unsafe { hg_draw_quad(self.dl, &q) }, OK);
    }

    /// begin + submit, lê o pixel, end. Devolve o pixel.
    fn draw_and_read(&self, x: u32, y: u32) -> u32 {
        let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let mut px = 0;
        unsafe {
            assert_eq!(hg_frame_begin(self.win, clear), OK, "{}", last_error());
            assert_eq!(hg_frame_submit(self.win, self.dl), OK, "{}", last_error());
            assert_eq!(hg__window_read_pixel(self.win, x, y, &mut px), OK, "{}", last_error());
            assert_eq!(hg_frame_end(self.win, 0), OK, "{}", last_error());
        }
        px
    }

    fn frame(&self) -> [i32; 3] {
        let clear = hg_color { r: 0.1, g: 0.1, b: 0.12, a: 1.0 };
        unsafe { [hg_frame_begin(self.win, clear), hg_frame_submit(self.win, self.dl), hg_frame_end(self.win, 0)] }
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

const RED: hg_color = hg_color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

#[test]
fn context_creates_with_wgpu() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    let info = unsafe { hg_context_backend_info(f.ctx) };
    assert_eq!(unsafe { hg_context_backend(f.ctx) }, WGPU);
    assert_ne!(info.adapter[0], 0, "nome do adaptador vazio");
    assert_eq!(f.frame(), [OK; 3]);
}

#[test]
fn solid_quad_pixel_is_red() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    f.quad(0.0, 0.0, 50.0, 50.0, RED);
    let px = f.draw_and_read(10, 10);
    assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "pixel = {px:#010x}");
    let mut stats = unsafe { std::mem::zeroed() };
    unsafe { hg_context_stats(f.ctx, &mut stats) };
    assert_eq!(stats.draw_calls, 1);
    assert!(stats.gpu_bytes > 0);
}

#[test]
fn text_only_first_frame_is_visible() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    let family = CString::new("Segoe UI").unwrap();
    let mut font = 0;
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let block = "█";
    unsafe {
        assert_eq!(hg_font_load(f.ctx, family.as_ptr(), 400.0, 0, &mut font), OK);
        assert_eq!(hg_push_clip(f.dl, hg_rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 }), OK);
        assert_eq!(hg_draw_text(f.dl, font, 96.0, 10.0, 10.0, 0.0, white, block.as_ptr().cast(), block.len()), OK);
    }
    let px = f.draw_and_read(40, 70);
    assert!(px & 0x00ff_ffff > 0x0080_8080, "pixel = {px:#010x}");
}

#[test]
fn resize_reconfigures_surface() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    unsafe { hg__window_inject_resize(f.win, 500, 400) };
    f.quad(0.0, 0.0, 500.0, 400.0, RED);
    let px = f.draw_and_read(480, 380);
    assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "pixel = {px:#010x}");
}

#[test]
fn zero_size_frame_is_noop() {
    if !wgpu_available() { return }
    let f = Fixture::new(WGPU);
    unsafe { hg__window_inject_resize(f.win, 0, 0) };
    assert_eq!(f.frame(), [OK; 3]);
    unsafe { hg__window_inject_resize(f.win, 200, 100) };
    assert_eq!(f.frame(), [OK; 3]);
}

#[test]
fn switch_d3d11_wgpu_d3d11_keeps_drawing() {
    if !wgpu_available() { return }
    let f = Fixture::new(hg_backend_kind_HG_BACKEND_D3D11);
    f.quad(0.0, 0.0, 50.0, 50.0, RED);
    for kind in [WGPU, hg_backend_kind_HG_BACKEND_D3D11, WGPU, hg_backend_kind_HG_BACKEND_D3D11] {
        assert_eq!(unsafe { hg_context_switch_backend(f.ctx, kind) }, OK, "{}", last_error());
        assert_eq!(unsafe { hg_context_backend(f.ctx) }, kind);
        let px = f.draw_and_read(10, 10);
        assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "{kind}: pixel = {px:#010x}");
    }
}

unsafe extern "C" fn dummy_probe(_: *mut hg_backend_info) -> i32 { 0 }

#[test]
fn register_twice_or_after_seal_fails() {
    wgpu_available(); // instala e sela o registro (enumerate)
    assert!(wgpu_backend::install(), "install é idempotente");
    let mut vt: hg_backend_vtbl = unsafe { std::mem::zeroed() };
    vt.kind = WGPU;
    vt.name = c"dup".as_ptr();
    vt.probe = Some(dummy_probe);
    let invalid = HG_ERR_INVALID as i32;
    assert_eq!(unsafe { hg__backend_register(ptr::addr_of!(vt).cast()) }, invalid);
    vt.kind = 99;
    assert_eq!(unsafe { hg__backend_register(ptr::addr_of!(vt).cast()) }, invalid);
}
