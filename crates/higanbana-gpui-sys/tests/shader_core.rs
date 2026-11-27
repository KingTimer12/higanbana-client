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

#[test]
fn shader_batch_inherits_current_clip() {
    let f = Fixture::new();
    let s = CString::new("x").unwrap();
    let clip = hg_rect { x: 10.0, y: 20.0, w: 30.0, h: 40.0 };
    let mut sh = ptr::null_mut();
    unsafe {
        assert_eq!(hg_shader_create(f.ctx, &code(&s), &mut sh), OK);
        assert_eq!(hg_push_clip(f.dl, clip), OK);
        assert_eq!(hg_draw_shader(f.dl, sh, &draw()), OK);
        hg_pop_clip(f.dl);
        let mut view: hg_batch_view = std::mem::zeroed();
        let mut cur = ptr::null();
        let mut found = false;
        loop {
            cur = hg__draw_list_next_batch(f.dl, cur, &mut view);
            if cur.is_null() { break }
            if !view.shader.is_null() {
                assert_ne!(view.has_clip, 0);
                assert_eq!((view.clip.x, view.clip.y, view.clip.w, view.clip.h), (10.0, 20.0, 30.0, 40.0));
                found = true;
            }
        }
        assert!(found, "lote de shader ausente");
        hg_shader_destroy(sh);
    }
}

#[test]
fn backend_switch_clears_native_error() {
    let f = Fixture::new();
    let s = CString::new("isto não compila").unwrap();
    let mut sh = ptr::null_mut();
    unsafe {
        assert_eq!(hg_shader_create(f.ctx, &code(&s), &mut sh), OK);
        let mut d = draw();
        d.rect = hg_rect { x: 10.0, y: 10.0, w: 50.0, h: 50.0 };
        assert_eq!(hg_draw_shader(f.dl, sh, &d), OK);
        assert_eq!(f.frame(), [OK, OK, OK], "erro de compilação não derruba o quadro");
        assert!(!hg_shader_error(sh).is_null(), "D3D11 grava o erro do compilador");
        // sai do D3D11: o erro era desse backend e não pode ficar velho
        if hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_D3D12) == OK
            || hg_context_switch_backend(f.ctx, hg_backend_kind_HG_BACKEND_OPENGL) == OK
        {
            assert!(hg_shader_error(sh).is_null());
        }
        hg_shader_destroy(sh);
    }
}
