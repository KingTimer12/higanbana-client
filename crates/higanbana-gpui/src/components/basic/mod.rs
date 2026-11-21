//! Componentes do grupo `basic`: Button, Badge, Card, Separator, Skeleton, Avatar.
//!
//! # Padrão dos componentes
//!
//! Todo componente é um *builder* de dados puro (`Button::new(..).variant(..)`) que
//! só vira [`Element`] em [`Component::render`], recebendo o `RenderCx`. Assim ele lê
//! os tokens do tema, o estado de foco (anel só com foco de teclado) e pode animar,
//! sem guardar referência ao contexto. Uso:
//!
//! ```ignore
//! use higanbana_gpui::components::*;
//! let ok = Button::new("salvar", "Salvar").on_click(|v: &mut App, _, cx| cx.notify()).render(cx);
//! div().child(ok)
//! ```
//!
//! Regras seguidas aqui (e esperadas dos grupos `form`/`overlay`):
//! - um arquivo por componente, cores por variante em funções puras testáveis;
//! - componentes interativos exigem um `id` (foco e animação sobrevivem às reconstruções);
//! - cores sempre do `Theme`, raio de `theme.radius`, alturas de `ComponentSize::height`;
//! - anel de foco só quando `cx.focus().visible` (foco veio do teclado);
//! - desabilitado = opacidade 0.5 + `.disabled(true)` (sem clique, foco nem hover).
mod avatar;
mod badge;
mod button;
mod card;
mod kbd;
mod link;
mod progress;
mod separator;
mod skeleton;
mod spinner;
mod tag;
mod variant_style;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_layout;
#[cfg(test)]
mod tests_progress;

pub use avatar::{initials, Avatar};
pub use badge::Badge;
pub use button::Button;
pub use card::{Card, CardHeader};
pub use kbd::Kbd;
pub use link::Link;
pub use progress::{Progress, ProgressSize};
pub use separator::{Orientation, Separator};
pub use skeleton::{Skeleton, PULSE_MS};
pub use spinner::Spinner;
pub use tag::Tag;
pub use variant_style::{variant_colors, VariantColors};

use crate::color::Color;
use crate::cx::RenderCx;
use crate::element::Element;
use crate::style::{Style, Styled};

/// Algo que vira um [`Element`] na hora do render, com acesso ao tema/foco/animações.
pub trait Component<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V>;
}

/// Anel de foco do shadcn (`ring-[3px] ring-ring/50 border-ring`) como sombra + borda.
pub fn focus_ring(s: Style, ring: Color) -> Style {
    s.shadow(ring.alpha(0.5), 3.0).border_color(ring)
}
