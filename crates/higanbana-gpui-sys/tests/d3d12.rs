use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;
mod common;
use common::OK;

const D3D12: hg_backend_kind = hg_backend_kind_HG_BACKEND_D3D12;

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
            let title = CString::new("d3d12").unwrap();
            let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 320, height: 240, resizable: 1 };
            let mut win = ptr::null_mut();
            assert_eq!(hg_window_create(ctx, &desc, &mut win), OK, "{:?}", CStr::from_ptr(hg_last_error()));
            Fx { ctx, win, dl: hg_draw_list_create() }
        }
    }

    fn quad(&self, color: hg_color) {
        unsafe {
            let q = hg_quad { rect: hg_rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }, fill: color, ..std::mem::zeroed() };
            assert_eq!(hg_draw_quad(self.dl, &q), OK);
        }
    }

    /// Desenha um frame e lê o pixel (x, y) antes do present.
    fn draw_read(&self, x: u32, y: u32) -> ([i32; 4], u32) {
        let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let mut px = 0;
        unsafe {
            let a = hg_frame_begin(self.win, clear);
            let b = hg_frame_submit(self.win, self.dl);
            let c = hg__window_read_pixel(self.win, x, y, &mut px);
            let d = hg_frame_end(self.win, 0);
            ([a, b, c, d], px)
        }
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

const GREEN: hg_color = hg_color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };

#[test]
fn context_uses_d3d12() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    assert_eq!(unsafe { hg_context_backend(f.ctx) }, D3D12);
}

#[test]
fn solid_quad_pixel_matches() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    f.quad(GREEN);
    for _ in 0..3 {
        let (rs, px) = f.draw_read(50, 50);
        assert_eq!(rs, [OK; 4]);
        assert_eq!(px & 0x00ff_ffff, 0x0000_ff00, "pixel {px:08x}");
        let (_, outside) = f.draw_read(200, 200);
        assert_eq!(outside & 0x00ff_ffff, 0, "fora do quad {outside:08x}");
    }
}

#[test]
fn text_is_visible() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    let family = CString::new("Segoe UI").unwrap();
    let mut font = 0;
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let block = "█";
    unsafe {
        assert_eq!(hg_font_load(f.ctx, family.as_ptr(), 400.0, 0, &mut font), OK);
        assert_eq!(hg_push_clip(f.dl, hg_rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 }), OK);
        assert_eq!(hg_draw_text(f.dl, font, 96.0, 10.0, 10.0, 0.0, white, block.as_ptr().cast(), block.len()), OK);
    }
    let (rs, px) = f.draw_read(40, 70);
    assert_eq!(rs, [OK; 4]);
    assert!(px & 0x00ff_ffff > 0x0080_8080, "pixel = {px:#010x}");
    let (rs, px) = f.draw_read(40, 70); // atlas já populado
    assert_eq!(rs, [OK; 4]);
    assert!(px & 0x00ff_ffff > 0x0080_8080, "pixel = {px:#010x}");
}

#[test]
fn resize_then_frame_ok() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    f.quad(GREEN);
    unsafe { hg__window_inject_resize(f.win, 640, 480) };
    let (rs, px) = f.draw_read(600, 400);
    assert_eq!(rs, [OK; 4]);
    assert_eq!(px & 0x00ff_ffff, 0, "{px:08x}");
    let (_, px) = f.draw_read(50, 50);
    assert_eq!(px & 0x00ff_ffff, 0x0000_ff00);
}

#[test]
fn zero_resize_is_noop() {
    if !d3d12_available() { return; }
    let f = Fx::new(D3D12);
    f.quad(GREEN);
    unsafe { hg__window_inject_resize(f.win, 0, 0) };
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    unsafe {
        assert_eq!(hg_frame_begin(f.win, clear), OK);
        assert_eq!(hg_frame_submit(f.win, f.dl), OK);
        assert_eq!(hg_frame_end(f.win, 0), OK);
        hg__window_inject_resize(f.win, 320, 240);
    }
    let (rs, px) = f.draw_read(50, 50);
    assert_eq!(rs, [OK; 4]);
    assert_eq!(px & 0x00ff_ffff, 0x0000_ff00);
}

#[test]
fn switch_d3d11_d3d12_d3d11_keeps_drawing() {
    if !d3d12_available() { return; }
    let f = Fx::new(hg_backend_kind_HG_BACKEND_D3D11);
    f.quad(GREEN);
    for kind in [D3D12, hg_backend_kind_HG_BACKEND_D3D11, D3D12, hg_backend_kind_HG_BACKEND_D3D11] {
        unsafe {
            assert_eq!(hg_context_switch_backend(f.ctx, kind), OK, "{:?}", CStr::from_ptr(hg_last_error()));
            assert_eq!(hg_context_backend(f.ctx), kind);
        }
        let (rs, px) = f.draw_read(50, 50);
        assert_eq!(rs, [OK; 4]);
        assert_eq!(px & 0x00ff_ffff, 0x0000_ff00, "{kind:?}");
    }
}
