//! Peças visuais sobre o vídeo (iguais às da master): scrims, cartão e cabeçalho de menu.

use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};

use crate::root::Higanbana;
use crate::theme::media_scrim;

/// Escurece de cima para baixo (barra de cima).
pub fn scrim_down(alpha: f32) -> LinearGradient {
    LinearGradient::new(GradientDir::Down, vec![(0.0, media_scrim(alpha)), (1.0, media_scrim(0.0))])
}

/// Escurece de baixo para cima (controles).
pub fn scrim_up(alpha: f32) -> LinearGradient {
    LinearGradient::new(GradientDir::Up, vec![(0.0, media_scrim(alpha)), (1.0, media_scrim(0.0))])
}

/// Sombra `shadow_lg`/`shadow_md` do kit.
pub const SHADOW_LG: f32 = 15.0;
pub const SHADOW_MD: f32 = 6.0;

/// Cartão sobre o vídeo (erros, aviso de rede, próximo episódio).
pub fn media_card(t: &Theme) -> Element<Higanbana> {
    div()
        .flex_col()
        .gap(8.0)
        .px(20.0)
        .py(16.0)
        .rounded(t.radius_lg)
        .border(1.0, t.border)
        .bg(t.popover)
        .text_color(t.popover_foreground)
        .shadow(Color::BLACK.alpha(0.1), SHADOW_LG)
}

pub fn menu_heading(t: &Theme, label: &'static str) -> Element<Higanbana> {
    div().pb(4.0).text_size(t.typography.xs).font_weight(500).text_color(t.muted_foreground).child(text(label))
}
