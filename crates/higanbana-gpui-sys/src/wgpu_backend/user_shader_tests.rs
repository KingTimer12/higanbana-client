//! Falha fora dos escopos de erro do wgpu (pânico do handler padrão) em `shader_prepare`:
//! o erro tem de ir para `sh->error` uma vez só, sem nova tentativa a cada quadro.

use super::user_shader::hooks;
use crate::*;
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::Ordering;

#[test]
fn panico_no_prepare_grava_erro_e_nao_repete() {
    assert!(super::install());
    let mut ctx = ptr::null_mut();
    let kind = hg_backend_kind_HG_BACKEND_WGPU;
    if unsafe { hg_context_create(kind, &mut ctx) } != HG_OK as hg_result || unsafe { hg_context_backend(ctx) } != kind {
        if !ctx.is_null() { unsafe { hg_context_destroy(ctx) } }
        eprintln!("WGPU indisponível: teste pulado");
        return;
    }
    let src = CString::new("fn shade() {}").unwrap();
    let code = hg_shader_code {
        hlsl: ptr::null(), glsl_vs: ptr::null(), glsl_fs: ptr::null(), wgsl: src.as_ptr(), spirv: ptr::null(), spirv_words: 0,
    };
    let mut sh = ptr::null_mut();
    assert_eq!(unsafe { hg_shader_create(ctx, &code, &mut sh) }, HG_OK as hg_result);

    hooks::PANIC.store(true, Ordering::SeqCst);
    let before = hooks::CALLS.load(Ordering::SeqCst);
    let first = unsafe { hg__shader_gpu(sh) };
    let second = unsafe { hg__shader_gpu(sh) };
    let calls = hooks::CALLS.load(Ordering::SeqCst) - before;

    assert_ne!(first, HG_OK as hg_result);
    assert_ne!(second, HG_OK as hg_result);
    assert_eq!(calls, 1, "shader_prepare não pode rodar de novo depois da falha");
    let err = unsafe { hg_shader_error(sh) };
    assert!(!err.is_null(), "sh->error vazio depois do pânico");
    let msg = unsafe { CStr::from_ptr(err) }.to_string_lossy().into_owned();
    assert!(msg.contains("wgpu"), "{msg}");
    assert!(unsafe { hg__shader_gpu_data(sh) }.is_null());
    unsafe {
        hg_shader_destroy(sh);
        hg_context_destroy(ctx);
    }
}
