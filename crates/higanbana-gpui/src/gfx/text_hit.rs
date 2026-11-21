//! Posições de cursor em texto de uma linha (pixels físicos, como `measure_text`).
use super::context::Context;
use super::font::Font;
use crate::error::{check, Result};
use higanbana_gpui_sys as sys;

impl Context {
    /// x de cada fronteira de caractere de `text` (codepoints + 1 valores) num só layout.
    /// Reaproveita `out` (limpo antes) para não alocar a cada frame.
    pub fn caret_positions(&self, font: Font, size_px: f32, text: &str, out: &mut Vec<f32>) -> Result<()> {
        out.clear();
        out.resize(text.chars().count() + 1, 0.0);
        check(unsafe {
            sys::hg_text_caret_positions(self.raw(), font.id, size_px, text.as_ptr().cast(), text.len(), out.as_mut_ptr(), out.len())
        })
    }

    /// Índice de byte (fronteira de caractere) mais próximo de `x`.
    pub fn text_hit_test(&self, font: Font, size_px: f32, text: &str, x: f32) -> Result<usize> {
        let mut b = 0usize;
        check(unsafe { sys::hg_text_hit_test(self.raw(), font.id, size_px, text.as_ptr().cast(), text.len(), x, &mut b) })?;
        Ok(b)
    }
}
