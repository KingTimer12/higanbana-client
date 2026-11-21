//! Skeleton do shadcn: bloco `muted` arredondado com pulsação de opacidade.
//!
//! Custo: só pede frames enquanto é renderizado. Saiu da árvore, ninguém mais
//! consulta o tween (o `Animator` o descarta) e o app volta a dormir.
use super::Component;
use crate::anim::{Easing, Transition};
use crate::cx::RenderCx;
use crate::element::{div, Element, ElementId};
use crate::style::{Length, Styled};
use std::time::Duration;

/// Meio ciclo da pulsação (1 -> 0.5 ou 0.5 -> 1), como `animate-pulse` (2 s o ciclo).
pub const PULSE_MS: u64 = 1000;
/// Quadros por segundo do pulso.
pub const PULSE_FPS: u16 = 24;

/// `Skeleton::new("capa").w(120.0).h(20.0)`; o `id` mantém a fase entre frames.
pub struct Skeleton {
    id: ElementId,
    width: Length,
    height: Length,
    round: bool,
}

impl Skeleton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), width: Length::Auto, height: Length::Auto, round: false }
    }

    pub fn w(mut self, l: impl Into<Length>) -> Self {
        self.width = l.into();
        self
    }

    pub fn h(mut self, l: impl Into<Length>) -> Self {
        self.height = l.into();
        self
    }

    /// Círculo (placeholder de avatar).
    pub fn rounded_full(mut self) -> Self {
        self.round = true;
        self
    }
}

/// Alvo da metade atual do ciclo, derivado do relógio (todos pulsam em fase).
fn pulse_target(now: std::time::Instant) -> f32 {
    let epoch = *EPOCH.get_or_init(std::time::Instant::now);
    let half = now.saturating_duration_since(epoch).as_millis() / PULSE_MS as u128;
    if half % 2 == 0 { 0.5 } else { 1.0 }
}

static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

impl<V> Component<V> for Skeleton {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let target = pulse_target(cx.now());
        // Pulso lento: 24 quadros/s bastam e poupam CPU (listas com vários esqueletos).
        let tr = Transition::new(Duration::from_millis(PULSE_MS), Easing::EaseInOut).fps(PULSE_FPS);
        let opacity = cx.animate_from(self.id, 1.0, target, tr);
        cx.request_redraw_in(Duration::from_millis(1000 / PULSE_FPS as u64));
        let theme = cx.theme();
        let radius = if self.round { 9999.0 } else { theme.radius - 2.0 };
        div().w(self.width).h(self.height).rounded(radius).bg(theme.muted).opacity(opacity)
    }
}
