//! Contexto do core: dono do device/backend, fontes e estatísticas.
use super::backend::{install_rust_backends, BackendInfo, BackendKind};
use super::font::{Font, FontCache, FontSpec, TextMetrics};
use crate::error::{check, Result};
use higanbana_gpui_sys as sys;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Estatísticas do último frame (ver `hg_stats`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub frame_ms: f32,
    pub gpu_bytes: u64,
    pub arena_bytes: u64,
    pub draw_calls: u32,
    pub atlas_entries: u32,
}

/// Contexto de GPU. Não é `Send`: o core é de uma thread só (a da UI).
/// Janelas e imagens emprestam o contexto e não podem sobreviver a ele.
pub struct Context {
    raw: NonNull<sys::hg_context>,
    fonts: RefCell<FontCache>,
    _not_send: PhantomData<*mut ()>,
}

impl Context {
    /// Cria o contexto preferindo `kind`; o core cai para outro backend se faltar.
    pub fn new(kind: BackendKind) -> Result<Self> {
        install_rust_backends();
        let mut raw = std::ptr::null_mut();
        check(unsafe { sys::hg_context_create(kind.to_raw(), &mut raw) })?;
        let raw = NonNull::new(raw).ok_or_else(|| crate::Error::invalid("hg_context_create devolveu nulo"))?;
        Ok(Self { raw, fonts: RefCell::new(FontCache::default()), _not_send: PhantomData })
    }

    pub(crate) fn raw(&self) -> *mut sys::hg_context {
        self.raw.as_ptr()
    }

    /// Backend ativo.
    pub fn backend(&self) -> BackendKind {
        BackendKind::from_raw(unsafe { sys::hg_context_backend(self.raw()) })
    }

    /// Nome e adaptador do backend ativo.
    pub fn backend_info(&self) -> BackendInfo {
        BackendInfo::from_raw(&unsafe { sys::hg_context_backend_info(self.raw()) })
    }

    /// Troca o backend sem fechar janelas. Em falha o anterior continua ativo.
    pub fn switch_backend(&self, kind: BackendKind) -> Result<()> {
        check(unsafe { sys::hg_context_switch_backend(self.raw(), kind.to_raw()) })
    }

    pub fn stats(&self) -> Stats {
        let mut s: sys::hg_stats = unsafe { std::mem::zeroed() };
        unsafe { sys::hg_context_stats(self.raw(), &mut s) };
        Stats {
            frame_ms: s.frame_ms,
            gpu_bytes: s.gpu_bytes,
            arena_bytes: s.arena_bytes,
            draw_calls: s.draw_calls,
            atlas_entries: s.atlas_entries,
        }
    }

    /// Carrega (ou devolve do cache) a fonte do sistema descrita por `spec`.
    pub fn font(&self, spec: &FontSpec) -> Result<Font> {
        self.fonts.borrow_mut().get_or_load(self.raw(), spec)
    }

    /// Mede texto em pixels físicos. `max_width <= 0` desliga a quebra de linha.
    pub fn measure_text(&self, font: Font, size_px: f32, text: &str, max_width: f32) -> Result<TextMetrics> {
        let mut m = sys::hg_text_metrics { width: 0.0, height: 0.0, baseline: 0.0 };
        check(unsafe {
            sys::hg_text_measure(self.raw(), font.id, size_px, text.as_ptr().cast(), text.len(), max_width, &mut m)
        })?;
        Ok(TextMetrics { width: m.width, height: m.height, baseline: m.baseline })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { sys::hg_context_destroy(self.raw()) };
    }
}
