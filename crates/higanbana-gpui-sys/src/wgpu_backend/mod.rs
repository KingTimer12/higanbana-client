//! Backend WGPU do higanbana-gpui, escrito em Rust com o crate `wgpu` e registrado no
//! core C em runtime (`hg__backend_register`).
//!
//! Uso: chamar [`install`] **antes** do primeiro `hg_enumerate_backends` /
//! `hg_context_create` do processo. Depois disso o registro do core está selado e
//! `install` devolve `false`.

mod buffers;
mod device;
mod ffi;
mod glyphs;
mod pipeline;
mod readback;
mod render;
mod surface;

use crate::*;
use std::ffi::{c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

/// Erro de um passo do backend: código `HG_ERR_*` + mensagem estática.
pub(crate) struct Fail(pub hg_result, pub &'static CStr);
pub(crate) type Res<T = ()> = Result<T, Fail>;

pub(crate) fn fail<T>(code: u32, msg: &'static CStr) -> Res<T> {
    Err(Fail(code as hg_result, msg))
}

/// Executa um callback chamado pelo C: pânico vira `HG_ERR_DEVICE` (nunca atravessa o FFI)
/// e `Err` vira `hg__fail`, preenchendo `hg_last_error`.
pub(crate) fn guard(f: impl FnOnce() -> Res) -> hg_result {
    let r = catch_unwind(AssertUnwindSafe(f))
        .unwrap_or(Err(Fail(HG_ERR_DEVICE as hg_result, c"wgpu: pânico interno")));
    match r {
        Ok(()) => HG_OK as hg_result,
        Err(Fail(code, msg)) => unsafe { hg__fail(code, msg.as_ptr()) },
    }
}

/// `hg_backend_vtbl` guarda ponteiros crus; é imutável e vive o processo inteiro.
struct SyncVtbl(hg_backend_vtbl);
unsafe impl Sync for SyncVtbl {}

static VTBL: SyncVtbl = SyncVtbl(hg_backend_vtbl {
    kind: hg_backend_kind_HG_BACKEND_WGPU,
    name: c"WGPU".as_ptr(),
    probe: Some(ffi::probe),
    create: Some(ffi::create),
    destroy: Some(ffi::destroy),
    attach: Some(ffi::attach),
    detach: Some(ffi::detach),
    resize: Some(ffi::resize),
    begin_frame: Some(ffi::begin_frame),
    submit: Some(ffi::submit),
    end_frame: Some(ffi::end_frame),
    texture_create: None, // imagens: fase 3 (agente do WGPU)
    texture_destroy: None,
    read_pixel: Some(ffi::read_pixel),
});

/// Registra o backend WGPU no core (idempotente). Deve rodar antes do primeiro
/// enumerate/criação de contexto. `true` se o backend ficou registrado.
pub fn install() -> bool {
    static ONCE: Once = Once::new();
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    ONCE.call_once(|| {
        let vt = &VTBL.0 as *const hg_backend_vtbl as *const c_void;
        let ok = unsafe { hg__backend_register(vt) } == HG_OK as hg_result;
        INSTALLED.store(ok, Ordering::SeqCst);
    });
    INSTALLED.load(Ordering::SeqCst)
}
