//! Estilo de texto: refinamento parcial (no elemento) e estilo resolvido (herdado).
use crate::color::Color;
use crate::gfx::FontSpec;
use std::rc::Rc;

/// Estilo de texto resolvido de um nó de texto.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub font: FontSpec,
    /// Tamanho em pixels lógicos.
    pub size: f32,
    pub color: Color,
    /// Quebra linhas na largura disponível.
    pub wrap: bool,
}

/// Campos opcionais: `None` herda do ancestral.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyleRefinement {
    pub family: Option<Rc<str>>,
    pub size: Option<f32>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub color: Option<Color>,
    pub wrap: Option<bool>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { font: FontSpec::default(), size: 14.0, color: Color::BLACK, wrap: true }
    }
}

impl TextStyleRefinement {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl TextStyle {
    /// Aplica o refinamento por cima deste estilo (herança).
    pub fn refined(&self, r: &TextStyleRefinement) -> TextStyle {
        if r.is_empty() {
            return self.clone();
        }
        TextStyle {
            font: FontSpec {
                family: r.family.clone().unwrap_or_else(|| self.font.family.clone()),
                weight: r.weight.unwrap_or(self.font.weight),
                italic: r.italic.unwrap_or(self.font.italic),
            },
            size: r.size.unwrap_or(self.size),
            color: r.color.unwrap_or(self.color),
            wrap: r.wrap.unwrap_or(self.wrap),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::rgb;

    #[test]
    fn refinement_overrides_only_set_fields() {
        let base = TextStyle::default();
        let r = TextStyleRefinement { size: Some(20.0), color: Some(rgb(0xff0000)), ..Default::default() };
        let t = base.refined(&r);
        assert_eq!(t.size, 20.0);
        assert_eq!(t.color, rgb(0xff0000));
        assert_eq!(t.font, base.font);
        assert!(t.wrap);
    }
}
