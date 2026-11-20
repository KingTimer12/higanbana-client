//! Backend OpenGL 3.3: pula se a máquina não tiver contexto 3.3 core.
mod common;
use common::OK;
use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;

const GL: hg_backend_kind = hg_backend_kind_HG_BACKEND_OPENGL;
const BG: hg_color = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
const RED: hg_color = hg_color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

fn opengl_available() -> bool {
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n].iter().any(|i| i.kind == GL)
}

fn err() -> String { unsafe { CStr::from_ptr(hg_last_error()).to_string_lossy().into_owned() } }

struct Gl { ctx: *mut hg_context, win: *mut hg_window, dl: *mut hg_draw_list }

impl Gl {
    fn new(kind: hg_backend_kind) -> Self {
        unsafe {
            let mut ctx = ptr::null_mut();
            assert_eq!(hg_context_create(kind, &mut ctx), OK, "{}", err());
            let title = CString::new("opengl").unwrap();
            let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 320, height: 240, resizable: 1 };
            let mut win = ptr::null_mut();
            assert_eq!(hg_window_create(ctx, &desc, &mut win), OK, "{}", err());
            Gl { ctx, win, dl: hg_draw_list_create() }
        }
    }

    fn quad(&self, x: f32, y: f32, w: f32, h: f32, radius: f32) {
        let q = hg_quad { rect: hg_rect { x, y, w, h }, fill: RED, radius: [radius; 4], ..unsafe { std::mem::zeroed() } };
        assert_eq!(unsafe { hg_draw_quad(self.dl, &q) }, OK);
    }

    /// begin + submit, lê o pixel e fecha o frame.
    fn draw_and_read(&self, x: u32, y: u32) -> u32 {
        unsafe {
            assert_eq!(hg_frame_begin(self.win, BG), OK, "{}", err());
            assert_eq!(hg_frame_submit(self.win, self.dl), OK, "{}", err());
            let mut px = 0;
            assert_eq!(hg__window_read_pixel(self.win, x, y, &mut px), OK, "{}", err());
            assert_eq!(hg_frame_end(self.win, 0), OK, "{}", err());
            hg_draw_list_reset(self.dl);
            px
        }
    }
}

impl Drop for Gl {
    fn drop(&mut self) {
        unsafe {
            hg_draw_list_destroy(self.dl);
            hg_window_destroy(self.win);
            hg_context_destroy(self.ctx);
        }
    }
}

#[test]
fn context_reports_opengl_and_renderer() {
    if !opengl_available() { return; }
    let g = Gl::new(GL);
    unsafe {
        assert_eq!(hg_context_backend(g.ctx), GL);
        let info = hg_context_backend_info(g.ctx);
        assert_ne!(info.adapter[0], 0, "GL_RENDERER vazio");
    }
}

#[test]
fn solid_quad_pixel_is_red() {
    if !opengl_available() { return; }
    let g = Gl::new(GL);
    g.quad(10.0, 10.0, 40.0, 40.0, 0.0);
    assert_eq!(g.draw_and_read(20, 20) & 0x00ff_ffff, 0x00ff_0000);
    g.quad(10.0, 10.0, 40.0, 40.0, 0.0);
    assert_eq!(g.draw_and_read(60, 60) & 0x00ff_ffff, 0, "fora do quad é fundo");
}

#[test]
fn rounded_corner_pixel_is_background() {
    if !opengl_available() { return; }
    let g = Gl::new(GL);
    g.quad(10.0, 10.0, 80.0, 80.0, 30.0);
    assert_eq!(g.draw_and_read(11, 11) & 0x00ff_ffff, 0);
    g.quad(10.0, 10.0, 80.0, 80.0, 30.0);
    assert_eq!(g.draw_and_read(50, 50) & 0x00ff_ffff, 0x00ff_0000);
}

#[test]
fn text_pixel_is_bright() {
    if !opengl_available() { return; }
    let g = Gl::new(GL);
    let family = CString::new("Segoe UI").unwrap();
    let mut font = 0;
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let block = "█";
    unsafe {
        assert_eq!(hg_font_load(g.ctx, family.as_ptr(), 400.0, 0, &mut font), OK);
        assert_eq!(hg_push_clip(g.dl, hg_rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 }), OK);
        assert_eq!(hg_draw_text(g.dl, font, 96.0, 10.0, 10.0, 0.0, white, block.as_ptr().cast(), block.len()), OK);
    }
    let px = g.draw_and_read(40, 70);
    assert!(px & 0x00ff_ffff > 0x0080_8080, "pixel = {px:#010x}");
}

#[test]
fn resize_moves_viewport_and_zero_is_noop() {
    if !opengl_available() { return; }
    let g = Gl::new(GL);
    unsafe { hg__window_inject_resize(g.win, 200, 150) };
    g.quad(180.0, 130.0, 20.0, 20.0, 0.0);
    assert_eq!(g.draw_and_read(190, 140) & 0x00ff_ffff, 0x00ff_0000);
    unsafe { hg__window_inject_resize(g.win, 0, 0) };
    g.quad(0.0, 0.0, 10.0, 10.0, 0.0);
    unsafe {
        assert_eq!(hg_frame_begin(g.win, BG), OK);
        assert_eq!(hg_frame_submit(g.win, g.dl), OK);
        assert_eq!(hg_frame_end(g.win, 0), OK);
        hg_draw_list_reset(g.dl);
        hg__window_inject_resize(g.win, 320, 240);
    }
    g.quad(300.0, 220.0, 20.0, 20.0, 0.0);
    assert_eq!(g.draw_and_read(310, 230) & 0x00ff_ffff, 0x00ff_0000);
}

#[test]
fn switch_d3d11_opengl_d3d11_keeps_drawing() {
    if !opengl_available() { return; }
    let g = Gl::new(hg_backend_kind_HG_BACKEND_D3D11);
    for kind in [GL, hg_backend_kind_HG_BACKEND_D3D11, GL, hg_backend_kind_HG_BACKEND_D3D11] {
        unsafe { assert_eq!(hg_context_switch_backend(g.ctx, kind), OK, "{}", err()) };
        g.quad(0.0, 0.0, 30.0, 30.0, 0.0);
        assert_eq!(g.draw_and_read(5, 5) & 0x00ff_ffff, 0x00ff_0000, "{kind:?}");
    }
}
