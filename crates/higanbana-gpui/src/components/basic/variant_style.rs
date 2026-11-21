//! Cores por variante (shadcn -> tokens do tema), compartilhadas por Button e Badge.
use crate::color::Color;
use crate::components::Variant;
use crate::theme::Theme;

/// Cores de uma variante: repouso e hover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariantColors {
    pub bg: Color,
    pub fg: Color,
    /// Borda de 1px (transparente quando a variante não tem borda).
    pub border: Color,
    pub hover_bg: Color,
    pub hover_fg: Color,
}

/// Primária/destrutiva ficam a 90% no hover, secundária a 80%,
/// outline/ghost ganham `accent`, link só muda a cor do texto.
pub fn variant_colors(theme: &Theme, variant: Variant) -> VariantColors {
    let t = Color::TRANSPARENT;
    let solid = |bg: Color, fg: Color, k: f32| VariantColors { bg, fg, border: t, hover_bg: bg.alpha(bg.a * k), hover_fg: fg };
    match variant {
        Variant::Default => solid(theme.primary, theme.primary_foreground, 0.9),
        Variant::Destructive => solid(theme.destructive, theme.destructive_foreground, 0.9),
        Variant::Secondary => solid(theme.secondary, theme.secondary_foreground, 0.8),
        Variant::Outline => VariantColors {
            bg: theme.background,
            fg: theme.foreground,
            border: theme.input,
            hover_bg: theme.accent,
            hover_fg: theme.accent_foreground,
        },
        Variant::Ghost => VariantColors { bg: t, fg: theme.foreground, border: t, hover_bg: theme.accent, hover_fg: theme.accent_foreground },
        Variant::Link => VariantColors { bg: t, fg: theme.primary, border: t, hover_bg: t, hover_fg: theme.primary.alpha(0.8) },
    }
}
