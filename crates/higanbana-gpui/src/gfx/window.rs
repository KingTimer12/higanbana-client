//! Janela nativa presa ao tempo de vida do `Context`.
use super::context::Context;
use super::draw::DrawList;
use super::event::Event;
use crate::color::Color;
use crate::error::{check, Error, Result};
use crate::geometry::{size, Size};
use higanbana_gpui_sys as sys;
use std::ffi::CString;
use std::ptr::NonNull;

/// Opções de criação da janela.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub title: String,
    /// Tamanho inicial em pixels lógicos.
    pub size: Size,
    pub resizable: bool,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self { title: "higanbana".into(), size: size(960.0, 640.0), resizable: true }
    }
}

/// Janela que empresta o contexto: não pode sobreviver a ele.
pub struct Window<'c> {
    raw: NonNull<sys::hg_window>,
    ctx: &'c Context,
}

impl<'c> Window<'c> {
    pub fn new(ctx: &'c Context, options: &WindowOptions) -> Result<Self> {
        let title = CString::new(options.title.as_str()).map_err(|_| Error::invalid("título com NUL"))?;
        let desc = sys::hg_window_desc {
            title_utf8: title.as_ptr(),
            width: options.size.width.max(1.0) as u32,
            height: options.size.height.max(1.0) as u32,
            resizable: options.resizable as i32,
        };
        let mut raw = std::ptr::null_mut();
        check(unsafe { sys::hg_window_create(ctx.raw(), &desc, &mut raw) })?;
        let raw = NonNull::new(raw).ok_or_else(|| Error::invalid("hg_window_create devolveu nulo"))?;
        Ok(Self { raw, ctx })
    }

    pub fn context(&self) -> &'c Context {
        self.ctx
    }

    /// Escala de DPI (físico = lógico * escala).
    pub fn scale(&self) -> f32 {
        let s = unsafe { sys::hg_window_dpi_scale(self.raw.as_ptr()) };
        if s > 0.0 { s } else { 1.0 }
    }

    /// Tamanho da área cliente em pixels físicos.
    pub fn physical_size(&self) -> (u32, u32) {
        let (mut w, mut h) = (0, 0);
        unsafe { sys::hg__window_size(self.raw.as_ptr(), &mut w, &mut h) };
        (w, h)
    }

    /// Tamanho da área cliente em pixels lógicos.
    pub fn logical_size(&self) -> Size {
        let (w, h) = self.physical_size();
        let s = self.scale();
        size(w as f32 / s, h as f32 / s)
    }

    /// Próximo evento sem bloquear.
    pub fn poll_event(&self) -> Option<Event> {
        let mut ev: sys::hg_event = unsafe { std::mem::zeroed() };
        while unsafe { sys::hg_window_poll_event(self.raw.as_ptr(), &mut ev) } != 0 {
            if let Some(e) = Event::from_raw(&ev) {
                return Some(e);
            }
        }
        None
    }

    /// Bloqueia até chegar um evento (ocioso não gasta CPU).
    pub fn wait_event(&self) -> Result<Event> {
        loop {
            let mut ev: sys::hg_event = unsafe { std::mem::zeroed() };
            check(unsafe { sys::hg_window_wait_event(self.raw.as_ptr(), &mut ev) })?;
            if let Some(e) = Event::from_raw(&ev) {
                return Ok(e);
            }
        }
    }

    /// Espera um evento por até `timeout`. `Ok(None)` = prazo esgotado.
    pub fn wait_event_timeout(&self, timeout: std::time::Duration) -> Result<Option<Event>> {
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        let mut ev: sys::hg_event = unsafe { std::mem::zeroed() };
        check(unsafe { sys::hg_window_wait_event_timeout(self.raw.as_ptr(), ms, &mut ev) })?;
        Ok(Event::from_raw(&ev).or_else(|| self.poll_event()))
    }

    /// Cursor do mouse sobre a área cliente.
    pub fn set_cursor(&self, cursor: super::cursor::CursorStyle) {
        unsafe { sys::hg_window_set_cursor(self.raw.as_ptr(), cursor.raw()) };
    }

    /// Desenha um frame: limpa com `clear`, envia `list` e apresenta.
    pub fn present(&self, clear: Color, list: &DrawList<'_>, vsync: bool) -> Result<()> {
        self.present_with(clear, list, vsync, |_| Ok(()))
    }

    /// Gancho de teste: desenha e lê pixels físicos (0x00RRGGBB) antes de apresentar.
    #[doc(hidden)]
    pub fn present_and_read(&self, clear: Color, list: &DrawList<'_>, points: &[(u32, u32)]) -> Result<Vec<u32>> {
        let mut out = Vec::with_capacity(points.len());
        self.present_with(clear, list, false, |win| {
            for &(x, y) in points {
                let mut px = 0u32;
                check(unsafe { sys::hg__window_read_pixel(win, x, y, &mut px) })?;
                out.push(px & 0x00ff_ffff);
            }
            Ok(())
        })?;
        Ok(out)
    }

    fn present_with(
        &self,
        clear: Color,
        list: &DrawList<'_>,
        vsync: bool,
        between: impl FnOnce(*mut sys::hg_window) -> Result<()>,
    ) -> Result<()> {
        let win = self.raw.as_ptr();
        check(unsafe { sys::hg_frame_begin(win, clear.to_hg()) })?;
        let submitted = check(unsafe { sys::hg_frame_submit(win, list.raw()) }).and_then(|_| between(win));
        let ended = check(unsafe { sys::hg_frame_end(win, vsync as i32) });
        submitted.and(ended)
    }
}

impl Drop for Window<'_> {
    fn drop(&mut self) {
        unsafe { sys::hg_window_destroy(self.raw.as_ptr()) };
    }
}
