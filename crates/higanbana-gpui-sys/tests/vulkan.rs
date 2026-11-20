//! Backend Vulkan: criação, pixel de quad/texto, resize, 0x0 e troca D3D11<->Vulkan.
//! Pula (retorna) se Vulkan não estiver disponível na máquina.
mod common;
use common::*;
use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;

const VULKAN: hg_backend_kind = hg_backend_kind_HG_BACKEND_VULKAN;

fn vulkan_available() -> bool {
    let mut infos: [hg_backend_info; 8] = unsafe { std::mem::zeroed() };
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), 8) } as usize;
    infos[..n].iter().any(|i| i.kind == VULKAN)
}

fn err() -> String {
    unsafe { CStr::from_ptr(hg_last_error()) }.to_string_lossy().into_owned()
}

/// Fixture já trocada para Vulkan.
fn vulkan_fixture() -> Option<Fixture> {
    if !vulkan_available() {
        return None;
    }
    let f = Fixture::new();
    unsafe {
        assert_eq!(hg_context_switch_backend(f.ctx, VULKAN), OK, "{}", err());
        assert_eq!(hg_context_backend(f.ctx), VULKAN);
    }
    Some(f)
}

fn red_quad(x: f32, y: f32) -> hg_quad {
    hg_quad { rect: hg_rect { x, y, w: 50.0, h: 50.0 },
              fill: hg_color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, ..unsafe { std::mem::zeroed() } }
}

/// begin + submit + leitura de pixel + end; devolve (resultados, pixel BGRA).
fn draw_and_read(f: &Fixture, px: (u32, u32)) -> ([i32; 4], u32) {
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut pixel = 0u32;
    unsafe {
        let r1 = hg_frame_begin(f.win, clear);
        let r2 = hg_frame_submit(f.win, f.dl);
        let r3 = hg__window_read_pixel(f.win, px.0, px.1, &mut pixel);
        let r4 = hg_frame_end(f.win, 0);
        ([r1, r2, r3, r4], pixel)
    }
}

#[test]
fn context_creates_with_vulkan() {
    if !vulkan_available() {
        return;
    }
    let mut ctx = ptr::null_mut();
    unsafe {
        assert_eq!(hg_context_create(VULKAN, &mut ctx), OK, "{}", err());
        assert_eq!(hg_context_backend(ctx), VULKAN);
        let info = hg_context_backend_info(ctx);
        assert_ne!(info.adapter[0], 0, "adapter vazio");
        eprintln!("vulkan: {:?}", CStr::from_ptr(info.adapter.as_ptr()));
        hg_context_destroy(ctx);
    }
}

#[test]
fn solid_quad_pixel_is_red() {
    let Some(f) = vulkan_fixture() else { return };
    unsafe { assert_eq!(hg_draw_quad(f.dl, &red_quad(0.0, 0.0)), OK) };
    let (rs, px) = draw_and_read(&f, (10, 10));
    assert_eq!(rs, [OK; 4], "{}", err());
    assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "pixel {px:08x}");
    // Fora do quad: cor de clear (preto).
    let (_, px) = draw_and_read(&f, (100, 100));
    assert_eq!(px & 0x00ff_ffff, 0, "pixel {px:08x}");
    let mut stats: hg_stats = unsafe { std::mem::zeroed() };
    unsafe { hg_context_stats(f.ctx, &mut stats) };
    assert!(stats.draw_calls >= 1 && stats.gpu_bytes > 0, "{stats:?}");
}

#[test]
fn text_pixel_is_bright() {
    let Some(f) = vulkan_fixture() else { return };
    let family = CString::new("Segoe UI").unwrap();
    let mut font = 0;
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let block = "█";
    unsafe {
        assert_eq!(hg_font_load(f.ctx, family.as_ptr(), 400.0, 0, &mut font), OK);
        assert_eq!(hg_push_clip(f.dl, hg_rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 }), OK);
        assert_eq!(hg_draw_text(f.dl, font, 96.0, 10.0, 10.0, 0.0, white, block.as_ptr().cast(), block.len()), OK);
    }
    let (rs, px) = draw_and_read(&f, (40, 70));
    assert_eq!(rs, [OK; 4], "{}", err());
    assert!(px & 0x00ff_ffff > 0x0080_8080, "pixel = {px:#010x}");
}

#[test]
fn resize_keeps_drawing() {
    let Some(f) = vulkan_fixture() else { return };
    unsafe {
        hg__window_inject_resize(f.win, 400, 300);
        assert_eq!(hg_draw_quad(f.dl, &red_quad(0.0, 0.0)), OK);
    }
    let (rs, px) = draw_and_read(&f, (10, 10));
    assert_eq!(rs, [OK; 4], "{}", err());
    assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "pixel {px:08x}");
    assert_eq!(f.frame(), [OK; 3]);
}

#[test]
fn zero_size_is_noop() {
    let Some(f) = vulkan_fixture() else { return };
    unsafe {
        hg__window_inject_resize(f.win, 0, 0);
        assert_eq!(hg_draw_quad(f.dl, &red_quad(0.0, 0.0)), OK);
    }
    assert_eq!(f.frame(), [OK; 3]);
    unsafe { hg__window_inject_resize(f.win, 320, 240) };
    assert_eq!(f.frame(), [OK; 3]);
}

#[test]
fn many_frames_cycle_frames_in_flight() {
    let Some(f) = vulkan_fixture() else { return };
    unsafe { assert_eq!(hg_draw_quad(f.dl, &red_quad(5.0, 5.0)), OK) };
    for _ in 0..6 {
        assert_eq!(f.frame(), [OK; 3], "{}", err());
    }
}

#[test]
fn switch_d3d11_vulkan_d3d11() {
    let Some(f) = vulkan_fixture() else { return };
    unsafe { assert_eq!(hg_draw_quad(f.dl, &red_quad(0.0, 0.0)), OK) };
    assert_eq!(f.frame(), [OK; 3]);
    unsafe {
        assert_eq!(hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D11), OK, "{}", err());
        assert_eq!(hg_context_backend(f.ctx), hg_backend_kind_HG_BACKEND_D3D11);
    }
    let (rs, px) = draw_and_read(&f, (10, 10));
    assert_eq!(rs, [OK; 4]);
    assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "pixel {px:08x}");
    unsafe { assert_eq!(hg_context_switch_backend(f.ctx, VULKAN), OK, "{}", err()) };
    let (rs, px) = draw_and_read(&f, (10, 10));
    assert_eq!(rs, [OK; 4], "{}", err());
    assert_eq!(px & 0x00ff_ffff, 0x00ff_0000, "pixel {px:08x}");
}
