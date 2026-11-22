//! Contexto do core: dono do device/backend, fontes e estatísticas.
use super::backend::{install_rust_backends, BackendInfo, BackendKind};
use super::font::{Font, FontCache, FontSpec, TextMetrics};
use crate::error::{check, Error, Result};
use std::ffi::CString;
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

thread_local! {
    /// Último contexto criado nesta thread (a da UI): o player cria o alvo de vídeo nele.
    static CURRENT: std::cell::Cell<*mut sys::hg_context> = const { std::cell::Cell::new(std::ptr::null_mut()) };
}

/// Contexto vivo desta thread, se houver (ver `CURRENT`).
pub(crate) fn current_raw() -> Option<NonNull<sys::hg_context>> {
    NonNull::new(CURRENT.with(|c| c.get()))
}

impl Context {
    /// Cria o contexto preferindo `kind`; o core cai para outro backend se faltar.
    pub fn new(kind: BackendKind) -> Result<Self> {
        install_rust_backends();
        let mut raw = std::ptr::null_mut();
        check(unsafe { sys::hg_context_create(kind.to_raw(), &mut raw) })?;
        let raw = NonNull::new(raw).ok_or_else(|| crate::Error::invalid("hg_context_create devolveu nulo"))?;
        CURRENT.with(|c| c.set(raw.as_ptr()));
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

    /// Soma um .ttf/.otf às famílias deste contexto (nada é instalado no Windows).
    pub fn add_font_file(&self, path: &std::path::Path) -> Result<()> {
        let p = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| Error::invalid("caminho com NUL"))?;
        check(unsafe { sys::hg_font_add_file(self.raw(), p.as_ptr()) })
    }

    /// A família existe (no sistema ou num arquivo somado)?
    pub fn has_font_family(&self, family: &str) -> bool {
        let Ok(c) = CString::new(family) else { return false };
        let mut out = 0;
        check(unsafe { sys::hg_font_family_exists(self.raw(), c.as_ptr(), &mut out) }).is_ok() && out != 0
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
        CURRENT.with(|c| {
            if c.get() == self.raw() {
                c.set(std::ptr::null_mut());
            }
        });
        unsafe { sys::hg_context_destroy(self.raw()) };
    }
}
