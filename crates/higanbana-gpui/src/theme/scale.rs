//! Escalas de espaçamento e tipografia (pixels lógicos).

/// Escala de espaçamento estilo Tailwind (1 unidade = 4 px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub unit: f32,
}

impl Spacing {
    /// `n` unidades: `s(4)` = 16 px.
    pub fn s(&self, n: f32) -> f32 {
        n * self.unit
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self { unit: 4.0 }
    }
}

/// Tamanhos de fonte (xs..xxl) e família padrão.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    pub family: &'static str,
    pub xs: f32,
    pub sm: f32,
    pub base: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self { family: "Segoe UI", xs: 12.0, sm: 14.0, base: 16.0, lg: 18.0, xl: 20.0, xxl: 24.0 }
    }
}
