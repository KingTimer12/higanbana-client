//! Tema por tokens no estilo shadcn/ui.
mod palette;
mod scale;

pub use scale::{Spacing, Typography};

use crate::color::Color;

/// Tokens de cor (nomes do shadcn), raio base e escalas.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub background: Color,
    pub foreground: Color,
    pub card: Color,
    pub card_foreground: Color,
    pub popover: Color,
    pub popover_foreground: Color,
    pub primary: Color,
    pub primary_foreground: Color,
    pub secondary: Color,
    pub secondary_foreground: Color,
    pub muted: Color,
    pub muted_foreground: Color,
    pub accent: Color,
    pub accent_foreground: Color,
    pub destructive: Color,
    pub destructive_foreground: Color,
    pub border: Color,
    pub input: Color,
    pub ring: Color,
    /// Raio base em pixels lógicos (`--radius`).
    pub radius: f32,
    pub spacing: Spacing,
    pub typography: Typography,
}

impl Theme {
    pub fn light() -> Self {
        palette::light()
    }

    pub fn dark() -> Self {
        palette::dark()
    }

    /// Tema oposto (claro <-> escuro).
    pub fn toggled(&self) -> Self {
        if self.dark { Self::light() } else { Self::dark() }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
