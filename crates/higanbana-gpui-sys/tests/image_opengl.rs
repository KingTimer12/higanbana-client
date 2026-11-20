//! Imagens no backend OpenGL 3.3: pula se a máquina não tiver contexto 3.3 core.
mod common;
use common::*;
use higanbana_gpui_sys::*;
use std::ptr;

const GL: hg_backend_kind = hg_backend_kind_HG_BACKEND_OPENGL;
const D3D11: hg_backend_kind = hg_backend_kind_HG_BACKEND_D3D11;
const WHITE: hg_color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

fn opengl_available() -> bool {
    let mut infos = [hg_backend_info { kind: 0, name: [0; 32], adapter: [0; 128] }; 8];
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n].iter().any(|i| i.kind == GL)
}

/// Fixture já trocada para OpenGL; None se indisponível.
fn gl_fixture() -> Option<Fixture> {
    if !opengl_available() {
        eprintln!("OpenGL 3.3 indisponível; pulando");
        return None;
    }
    let f = Fixture::new();
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, GL), OK) };
    Some(f)
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

fn rect(x: f32, y: f32, w: f32, h: f32) -> hg_rect { hg_rect { x, y, w, h } }

fn pixel(f: &Fixture, x: u32, y: u32) -> u32 {
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut px = 0;
    unsafe {
        assert_eq!(hg_frame_begin(f.win, clear), OK);
        assert_eq!(hg_frame_submit(f.win, f.dl), OK, "{:?}", std::ffi::CStr::from_ptr(hg_last_error()));
        assert_eq!(hg__window_read_pixel(f.win, x, y, &mut px), OK);
        assert_eq!(hg_frame_end(f.win, 0), OK);
    }
    px & 0x00ff_ffff
}

fn stats(f: &Fixture) -> hg_stats {
    let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    s
}

#[test]
fn gl_image_pixels_reach_screen_with_tint() {
    let Some(f) = gl_fixture() else { return };
    let img = solid_image(f.ctx, [0, 255, 0, 255], 4, 4);
    let half = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 0.5 };
    unsafe {
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 0.0, 40.0, 40.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(50.0, 0.0, 40.0, 40.0), half, 0.0)), OK);
    }
    assert_eq!(pixel(&f, 20, 20), 0x0000_ff00);
    let g = (pixel(&f, 70, 20) >> 8) & 0xff;
    assert!((120..=136).contains(&g), "meio alfa: {g}");
    let s = stats(&f);
    assert_eq!(s.draw_calls, 1, "mesma imagem vizinha = um draw");
    assert!(s.gpu_bytes >= 4 * 4 * 4, "textura conta em gpu_bytes");
    unsafe { hg_image_destroy(img) };
}

#[test]
fn gl_different_images_split_draw_calls() {
    let Some(f) = gl_fixture() else { return };
    let a = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    let b = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    unsafe {
        assert_eq!(hg_draw_image(f.dl, a, &draw(rect(0.0, 0.0, 20.0, 20.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, a, &draw(rect(20.0, 0.0, 20.0, 20.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, b, &draw(rect(40.0, 0.0, 20.0, 20.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, a, &draw(rect(60.0, 0.0, 20.0, 20.0), WHITE, 0.0)), OK);
    }
    assert_eq!(pixel(&f, 30, 10), 0x00ff_0000);
    assert_eq!(pixel(&f, 50, 10), 0x0000_00ff);
    assert_eq!(pixel(&f, 70, 10), 0x00ff_0000, "instância após troca de textura");
    assert_eq!(stats(&f).draw_calls, 3, "a,a | b | a");
    unsafe {
        hg_image_destroy(a);
        hg_image_destroy(b);
    }
}

#[test]
fn gl_rounded_image_corner_is_background() {
    let Some(f) = gl_fixture() else { return };
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 0.0, 60.0, 60.0), WHITE, 30.0)), OK) };
    assert_eq!(pixel(&f, 1, 1), 0);
    assert_eq!(pixel(&f, 30, 30), 0x00ff_0000);
    unsafe { hg_image_destroy(img) };
}

#[test]
fn gl_layer_puts_quad_over_image() {
    let Some(f) = gl_fixture() else { return };
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe {
        let r = rect(0.0, 0.0, 60.0, 60.0);
        let q = hg_quad { rect: r, fill: hg_color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }, ..std::mem::zeroed() };
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(r, WHITE, 0.0)), OK);
        assert_eq!(pixel(&f, 30, 30), 0x00ff_0000, "imagem sobre quad na mesma camada");
        assert_eq!(hg_draw_list_layer(f.dl), OK);
        assert_eq!(hg_draw_quad(f.dl, &q), OK);
        assert_eq!(pixel(&f, 30, 30), 0x0000_00ff, "camada nova fica por cima");
        hg_image_destroy(img);
    }
}

#[test]
fn gl_image_survives_switch_d3d11_opengl_d3d11() {
    if !opengl_available() {
        eprintln!("OpenGL 3.3 indisponível; pulando");
        return;
    }
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    let _leak = solid_image(f.ctx, [1, 2, 3, 255], 2, 2); // liberada por hg_context_destroy
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 0.0, 40.0, 40.0), WHITE, 0.0)), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "D3D11");
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, GL), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "OpenGL");
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, D3D11), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "D3D11 de novo");
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, GL), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "OpenGL de novo");
    unsafe { hg_image_destroy(img) };
}
