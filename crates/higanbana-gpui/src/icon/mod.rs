//! Ícones SVG: `icon("play")` desenha o ícone na cor do texto herdada, 16px por padrão
//! (`.size(20.0)` muda). Rasterizados em pixels físicos e guardados em cache.
mod lucide;
pub mod raster;
mod registry;
#[cfg(test)]
mod tests;

pub use registry::{exists, register, IconSet};

use crate::element::{element, Element, Kind};
use crate::style::{Style, Styled};

/// Tamanho padrão (lado) em pixels lógicos.
pub const DEFAULT_SIZE: f32 = 16.0;

/// Ícone registrado por nome (Lucide em kebab-case ou um do `IconSet` do app).
pub fn icon<V>(name: &'static str) -> Element<V> {
    element(Kind::Icon(name), Style::default()).size(DEFAULT_SIZE).flex_shrink_0()
}
