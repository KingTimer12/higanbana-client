//! Tema por tokens no estilo shadcn/ui.
mod palette;
mod scale;
mod json;
#[cfg(test)]
mod tests;

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
    /// Primária sob hover / pressionada.
    pub primary_hover: Color,
    pub primary_active: Color,
    /// Superfície agrupadora (cards de menu, caixas).
    pub group_box: Color,
    /// Fundo de item de lista sob o mouse / selecionado, e a borda do selecionado.
    pub list_hover: Color,
    pub list_active: Color,
    pub list_active_border: Color,
    pub success: Color,
    pub success_foreground: Color,
    pub warning: Color,
    pub warning_foreground: Color,
    pub info: Color,
    pub info_foreground: Color,
    pub link: Color,
    pub selection: Color,
    /// Fundo escurecido atrás de diálogos.
    pub overlay: Color,
    pub sidebar: Color,
    pub sidebar_foreground: Color,
    pub sidebar_border: Color,
    pub sidebar_accent: Color,
    pub sidebar_accent_foreground: Color,
    pub skeleton: Color,
    pub switch: Color,
    pub scrollbar_thumb: Color,
    /// Cores-base nomeadas (gráficos, destaques).
    pub base: BaseColors,
    /// Raio base em pixels lógicos (`--radius`).
    pub radius: f32,
    /// Raio maior (cards, popovers, menus).
    pub radius_lg: f32,
    pub spacing: Spacing,
    pub typography: Typography,
}

/// Cores-base nomeadas e suas variantes claras.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseColors {
    pub red: Color,
    pub red_light: Color,
    pub green: Color,
    pub green_light: Color,
    pub blue: Color,
    pub blue_light: Color,
    pub yellow: Color,
    pub yellow_light: Color,
    pub magenta: Color,
    pub magenta_light: Color,
    pub cyan: Color,
    pub cyan_light: Color,
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
