//! Formas do cursor do mouse (aplicadas pela janela sobre a área cliente).
use higanbana_gpui_sys as sys;

/// Cursor do mouse. `.cursor(CursorStyle::Pointer)` no elemento aplica no hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorStyle {
    #[default]
    Arrow,
    /// Mão (links, botões).
    Pointer,
    /// Barra de texto.
    Text,
    Crosshair,
    NotAllowed,
    ResizeHorizontal,
    ResizeVertical,
    Move,
    Wait,
}

impl CursorStyle {
    pub(crate) fn raw(self) -> sys::hg_cursor {
        match self {
            CursorStyle::Arrow => sys::hg_cursor_HG_CURSOR_ARROW,
            CursorStyle::Pointer => sys::hg_cursor_HG_CURSOR_HAND,
            CursorStyle::Text => sys::hg_cursor_HG_CURSOR_IBEAM,
            CursorStyle::Crosshair => sys::hg_cursor_HG_CURSOR_CROSSHAIR,
            CursorStyle::NotAllowed => sys::hg_cursor_HG_CURSOR_NOT_ALLOWED,
            CursorStyle::ResizeHorizontal => sys::hg_cursor_HG_CURSOR_RESIZE_EW,
            CursorStyle::ResizeVertical => sys::hg_cursor_HG_CURSOR_RESIZE_NS,
            CursorStyle::Move => sys::hg_cursor_HG_CURSOR_MOVE,
            CursorStyle::Wait => sys::hg_cursor_HG_CURSOR_WAIT,
        }
    }
}
