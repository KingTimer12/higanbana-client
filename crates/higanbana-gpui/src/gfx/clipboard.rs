//! Área de transferência do sistema (texto UTF-8).
use crate::error::{check, Result};
use higanbana_gpui_sys as sys;

/// Copia `text` para a área de transferência.
pub fn set_text(text: &str) -> Result<()> {
    check(unsafe { sys::hg_clipboard_set_text(text.as_ptr().cast(), text.len()) })
}

/// Texto atual da área de transferência (`None` se vazia ou sem texto).
pub fn get_text() -> Option<String> {
    let mut len = 0usize;
    check(unsafe { sys::hg_clipboard_get_text(std::ptr::null_mut(), 0, &mut len) }).ok()?;
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    check(unsafe { sys::hg_clipboard_get_text(buf.as_mut_ptr().cast(), buf.len(), &mut len) }).ok()?;
    // Outro processo pode ter trocado o conteúdo entre as duas chamadas.
    buf.truncate(len.min(buf.len()));
    String::from_utf8(buf).ok()
}
