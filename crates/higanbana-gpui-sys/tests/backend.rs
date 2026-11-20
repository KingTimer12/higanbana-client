use higanbana_gpui_sys::*;
use std::ptr;

fn empty_info() -> hg_backend_info {
    unsafe { std::mem::zeroed() }
}

#[test]
fn enumerate_does_not_crash_and_lists_d3d11_on_windows() {
    let mut out = [empty_info(); 8];
    let n = unsafe { hg_enumerate_backends(out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 1);
    assert_eq!(out[0].kind, hg_backend_kind_HG_BACKEND_D3D11);
}

fn create(kind: hg_backend_kind) -> (*mut hg_context, hg_backend_kind) {
    let mut ctx = ptr::null_mut();
    let r = unsafe { hg_context_create(kind, &mut ctx) };
    assert_eq!(r, HG_OK as i32);
    let real = unsafe { hg_context_backend(ctx) };
    (ctx, real)
}

#[test]
fn unavailable_preferred_falls_back() {
    let (ctx, real) = create(hg_backend_kind_HG_BACKEND_VULKAN);
    assert_eq!(real, hg_backend_kind_HG_BACKEND_D3D11);
    unsafe { hg_context_destroy(ctx) };
}

#[test]
fn auto_picks_d3d11() {
    let (ctx, real) = create(hg_backend_kind_HG_BACKEND_AUTO);
    assert_eq!(real, hg_backend_kind_HG_BACKEND_D3D11);
    unsafe { hg_context_destroy(ctx) };
}
