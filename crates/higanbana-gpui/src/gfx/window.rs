//! Janela nativa presa ao tempo de vida do `Context`.
use super::context::Context;
use super::draw::DrawList;
use super::decor::{Decorations, HitKind, HitRegion, WindowCommand, WindowState};
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
    /// Barra nativa ou desenhada pelo app.
    pub decorations: Decorations,
    /// Tamanho mínimo da área cliente em pixels lógicos.
    pub min_size: Option<Size>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self { title: "higanbana".into(), size: size(960.0, 640.0), resizable: true, decorations: Decorations::Native, min_size: None }
    }
}

/// Janela que empresta o contexto: não pode sobreviver a ele.
pub struct Window<'c> {
    raw: NonNull<sys::hg_window>,
    ctx: &'c Context,
    waker: super::waker::Waker,
    wake_target: std::sync::Arc<super::waker::WakeTarget>,
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
        let (waker, wake_target) = super::waker::Waker::new(raw.as_ptr());
        let win = Self { raw, ctx, waker, wake_target };
        if options.decorations == Decorations::Custom {
            check(unsafe { sys::hg_window_set_decorations(win.raw.as_ptr(), 1) })?;
        }
        if let Some(m) = options.min_size {
            check(unsafe { sys::hg_window_set_min_size(win.raw.as_ptr(), m.width, m.height) })?;
        }
        Ok(win)
    }

    /// Handle nativo (HWND no Windows) para integrações do app: teclas de mídia,
    /// atributos do DWM. Pode mudar se o backend trocar para OpenGL.
    pub fn native_handle(&self) -> usize {
        unsafe { sys::hg__window_hwnd(self.raw.as_ptr()) as usize }
    }

    /// Publica as regiões de hit test (arrastar, maximizar, cliente).
    pub fn set_hit_regions(&self, regions: &[HitRegion]) {
        let raw: Vec<sys::hg_hit_region> = regions
            .iter()
            .map(|r| sys::hg_hit_region {
                x: r.rect.origin.x,
                y: r.rect.origin.y,
                w: r.rect.size.width,
                h: r.rect.size.height,
                kind: match r.kind {
                    HitKind::Client => sys::hg_hit_HG_HIT_CLIENT,
                    HitKind::Caption => sys::hg_hit_HG_HIT_CAPTION,
                    HitKind::Maximize => sys::hg_hit_HG_HIT_MAXIMIZE,
                },
            })
            .collect();
        unsafe { sys::hg_window_set_hit_regions(self.raw.as_ptr(), raw.as_ptr(), raw.len() as u32) };
    }

    /// Minimizar, maximizar, fechar, fullscreen...
    pub fn command(&self, cmd: WindowCommand) {
        use WindowCommand::*;
        let c = match cmd {
            Minimize => sys::hg_window_cmd_HG_WINDOW_MINIMIZE,
            Maximize => sys::hg_window_cmd_HG_WINDOW_MAXIMIZE,
            Restore => sys::hg_window_cmd_HG_WINDOW_RESTORE,
            ToggleMaximize => sys::hg_window_cmd_HG_WINDOW_TOGGLE_MAXIMIZE,
            Close => sys::hg_window_cmd_HG_WINDOW_CLOSE,
            EnterFullscreen => sys::hg_window_cmd_HG_WINDOW_ENTER_FULLSCREEN,
            ExitFullscreen => sys::hg_window_cmd_HG_WINDOW_EXIT_FULLSCREEN,
            ToggleFullscreen if self.state().fullscreen => sys::hg_window_cmd_HG_WINDOW_EXIT_FULLSCREEN,
            ToggleFullscreen => sys::hg_window_cmd_HG_WINDOW_ENTER_FULLSCREEN,
        };
        unsafe { sys::hg_window_command(self.raw.as_ptr(), c) };
    }

    pub fn state(&self) -> WindowState {
        WindowState::from_flags(unsafe { sys::hg_window_state(self.raw.as_ptr()) })
    }

    /// Acorda o laço desta janela de qualquer thread (vira no-op depois do drop).
    pub fn waker(&self) -> super::waker::Waker {
        self.waker.clone()
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
        self.wake_target.clear();
        unsafe { sys::hg_window_destroy(self.raw.as_ptr()) };
    }
}
