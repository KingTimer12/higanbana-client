//! Fontes do sistema (DirectWrite no core) e cache por família/peso/itálico.
use crate::error::{check, Error, Result};
use higanbana_gpui_sys as sys;
use std::ffi::CString;
use std::rc::Rc;

/// Fonte carregada no contexto. Barata de copiar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Font {
    pub(crate) id: sys::hg_font_id,
}

/// Descrição de uma fonte: família, peso (100..900) e itálico.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontSpec {
    pub family: Rc<str>,
    pub weight: u16,
    pub italic: bool,
}

/// Medidas de um texto (mesma unidade em que foi medido).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    /// Distância do topo até a linha de base da primeira linha.
    pub baseline: f32,
}

impl FontSpec {
    pub fn new(family: impl Into<Rc<str>>, weight: u16, italic: bool) -> Self {
        Self { family: family.into(), weight, italic }
    }
}

impl Default for FontSpec {
    fn default() -> Self {
        Self::new("Segoe UI", 400, false)
    }
}

/// Poucas fontes por app: busca linear num Vec evita alocar chave a cada consulta.
#[derive(Default)]
pub(crate) struct FontCache {
    entries: Vec<(FontSpec, Font)>,
}

impl FontCache {
    pub(crate) fn get_or_load(&mut self, ctx: *mut sys::hg_context, spec: &FontSpec) -> Result<Font> {
        if let Some((_, f)) = self.entries.iter().find(|(s, _)| s == spec) {
            return Ok(*f);
        }
        let family = CString::new(&*spec.family).map_err(|_| Error::invalid("família com NUL"))?;
        let mut id = 0;
        check(unsafe { sys::hg_font_load(ctx, family.as_ptr(), spec.weight as f32, spec.italic as i32, &mut id) })?;
        let font = Font { id };
        self.entries.push((spec.clone(), font));
        Ok(font)
    }
}
