mod common;
use common::*;
use higanbana_gpui_sys::*;
use std::ffi::CString;
use std::ptr;

fn code(s: &CString) -> hg_shader_code {
    hg_shader_code { hlsl: s.as_ptr(), glsl_vs: s.as_ptr(), glsl_fs: s.as_ptr(), wgsl: s.as_ptr(), spirv: ptr::null(), spirv_words: 0 }
}

fn draw() -> hg_shader_draw {
    unsafe { std::mem::zeroed() }
}

fn quad() -> hg_quad {
    unsafe { std::mem::zeroed() }
}

/// Lotes de quads não vazios e lotes de shader, na ordem.
fn kinds(dl: *const hg_draw_list) -> Vec<&'static str> {
    let mut v = Vec::new();
    let mut view: hg_batch_view = unsafe { std::mem::zeroed() };
    let mut cur = ptr::null();
    loop {
        cur = unsafe { hg__draw_list_next_batch(dl, cur, &mut view) };
        if cur.is_null() { break }
        if !view.shader.is_null() { v.push("shader") } else if view.quad_count > 0 { v.push("quad") }
    }
    v
}

#[test]
fn create_copies_code_and_reports_no_error() {
    let f = Fixture::new();
    let s = CString::new("x").unwrap();
    let mut sh = ptr::null_mut();
    unsafe {
        assert_eq!(hg_shader_create(f.ctx, &code(&s), &mut sh), OK);
        assert!(!sh.is_null());
        assert!(hg_shader_error(sh).is_null());
        hg_shader_destroy(sh);
    }
}

#[test]
fn draw_shader_sits_between_quads() {
    let f = Fixture::new();
    let s = CString::new("x").unwrap();
    let mut sh = ptr::null_mut();
    unsafe {
        assert_eq!(hg_shader_create(f.ctx, &code(&s), &mut sh), OK);
        assert_eq!(hg_draw_quad(f.dl, &quad()), OK);
        assert_eq!(hg_draw_shader(f.dl, sh, &draw()), OK);
        assert_eq!(hg_draw_quad(f.dl, &quad()), OK);
        assert_eq!(kinds(f.dl), ["quad", "shader", "quad"]);
        // o backend não tem shader_prepare: o frame segue sem desenhar o lote
        assert_eq!(f.frame(), [OK, OK, OK]);
        hg_shader_destroy(sh);
    }
}

#[test]
fn draw_shader_rejects_null_shader() {
    let f = Fixture::new();
    unsafe { assert_eq!(hg_draw_shader(f.dl, ptr::null_mut(), &draw()), HG_ERR_INVALID as i32) };
}

#[test]
fn context_destroy_with_live_shader_is_clean() {
    let s = CString::new("x").unwrap();
    unsafe {
        let mut ctx = ptr::null_mut();
        assert_eq!(hg_context_create(hg_backend_kind_HG_BACKEND_D3D11, &mut ctx), OK);
        let mut sh = ptr::null_mut();
        assert_eq!(hg_shader_create(ctx, &code(&s), &mut sh), OK);
        hg_context_destroy(ctx);
    }
}
