//! Imagens no backend Vulkan: pixels + tint, agrupamento, canto arredondado,
//! camadas, troca D3D11 -> Vulkan -> D3D11 e destruição adiada.
//! Pula (retorna) se Vulkan não estiver disponível na máquina.
mod common;
use common::*;
use higanbana_gpui_sys::*;
use std::ffi::CStr;
use std::ptr;

const VULKAN: hg_backend_kind = hg_backend_kind_HG_BACKEND_VULKAN;
const WHITE: hg_color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

fn err() -> String {
    unsafe { CStr::from_ptr(hg_last_error()) }.to_string_lossy().into_owned()
}

fn vulkan_available() -> bool {
    let mut infos: [hg_backend_info; 8] = unsafe { std::mem::zeroed() };
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n].iter().any(|i| i.kind == VULKAN)
}

fn vulkan_fixture() -> Option<Fixture> {
    if !vulkan_available() {
        return None;
    }
    let f = Fixture::new();
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, VULKAN), OK, "{}", err()) };
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

fn rect(x: f32, w: f32) -> hg_rect {
    hg_rect { x, y: 0.0, w, h: w }
}

fn pixel(f: &Fixture, x: u32, y: u32) -> u32 {
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut px = 0;
    unsafe {
        assert_eq!(hg_frame_begin(f.win, clear), OK, "{}", err());
        assert_eq!(hg_frame_submit(f.win, f.dl), OK, "{}", err());
        assert_eq!(hg__window_read_pixel(f.win, x, y, &mut px), OK, "{}", err());
        assert_eq!(hg_frame_end(f.win, 0), OK, "{}", err());
    }
    px & 0x00ff_ffff
}

fn draw_calls(f: &Fixture) -> u32 {
    let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    s.draw_calls
}

#[test]
fn vulkan_image_pixels_with_tint_and_grouping() {
    let Some(f) = vulkan_fixture() else { return };
    let img = solid_image(f.ctx, [0, 255, 0, 255], 4, 4);
    let half = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 0.5 };
    unsafe {
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 40.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, img, &draw(rect(50.0, 40.0), half, 0.0)), OK);
    }
    assert_eq!(pixel(&f, 20, 20), 0x0000_ff00);
    let g = (pixel(&f, 70, 20) >> 8) & 0xff;
    assert!((120..=136).contains(&g), "meio alfa: {g}");
    assert_eq!(draw_calls(&f), 1, "mesma imagem vizinha = um draw");
    let mut s = unsafe { std::mem::zeroed::<hg_stats>() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    assert!(s.gpu_bytes >= 4 * 4 * 4, "textura conta em gpu_bytes");
    unsafe { hg_image_destroy(img) };
}

#[test]
fn vulkan_alternating_images_split_draws() {
    let Some(f) = vulkan_fixture() else { return };
    let a = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    let b = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    unsafe {
        assert_eq!(hg_draw_image(f.dl, a, &draw(rect(0.0, 20.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, b, &draw(rect(30.0, 20.0), WHITE, 0.0)), OK);
        assert_eq!(hg_draw_image(f.dl, a, &draw(rect(60.0, 20.0), WHITE, 0.0)), OK);
    }
    assert_eq!(pixel(&f, 40, 10), 0x0000_00ff);
    assert_eq!(pixel(&f, 70, 10), 0x00ff_0000);
    assert_eq!(draw_calls(&f), 3);
    unsafe {
        hg_image_destroy(a);
        hg_image_destroy(b);
    }
}

#[test]
fn vulkan_rounded_image_corner_is_background() {
    let Some(f) = vulkan_fixture() else { return };
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 60.0), WHITE, 30.0)), OK) };
    assert_eq!(pixel(&f, 1, 1), 0);
    assert_eq!(pixel(&f, 30, 30), 0x00ff_0000);
    unsafe { hg_image_destroy(img) };
}

#[test]
fn vulkan_layer_puts_quad_over_image() {
    let Some(f) = vulkan_fixture() else { return };
    let img = solid_image(f.ctx, [255, 0, 0, 255], 2, 2);
    unsafe {
        let r = rect(0.0, 60.0);
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
fn vulkan_destroyed_image_is_released_after_frames() {
    let Some(f) = vulkan_fixture() else { return };
    for i in 0..20u8 {
        let img = solid_image(f.ctx, [i, 0, 255, 255], 8, 8);
        unsafe {
            hg_draw_list_reset(f.dl);
            assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 20.0), WHITE, 0.0)), OK);
        }
        assert_eq!(pixel(&f, 10, 10) & 0xff, 0xff);
        unsafe { hg_image_destroy(img) }; // GPU pode ainda usar: liberação adiada
    }
}

#[test]
fn vulkan_image_survives_switch_d3d11_vulkan_d3d11() {
    if !vulkan_available() {
        return;
    }
    let f = Fixture::new();
    let img = solid_image(f.ctx, [0, 0, 255, 255], 2, 2);
    unsafe { assert_eq!(hg_draw_image(f.dl, img, &draw(rect(0.0, 40.0), WHITE, 0.0)), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff);
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, VULKAN), OK, "{}", err()) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "textura recriada no Vulkan");
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK) };
    assert_eq!(pixel(&f, 20, 20), 0x0000_00ff, "textura recriada no D3D11");
    unsafe { hg_image_destroy(img) };
}
