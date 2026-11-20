//! Curvas de suavização e a descrição de uma transição.
use std::time::Duration;

/// Curva aplicada ao progresso `t` (0..1).
#[derive(Debug, Clone, Copy)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Função própria (deve mapear 0 -> 0 e 1 -> 1).
    Custom(fn(f32) -> f32),
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Custom(f) => f(t),
        }
    }
}

/// Duração + curva. `Transition::default()` = 150 ms, ease-out (padrão shadcn).
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub duration: Duration,
    pub easing: Easing,
}

impl Transition {
    pub const fn new(duration: Duration, easing: Easing) -> Self {
        Self { duration, easing }
    }

    pub const fn ms(ms: u64) -> Self {
        Self { duration: Duration::from_millis(ms), easing: Easing::EaseOut }
    }

    pub const fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::ms(150)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_hit_endpoints() {
        for e in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
            assert_eq!(e.apply(0.0), 0.0);
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
            assert!(e.apply(0.5) > 0.0 && e.apply(0.5) < 1.0);
        }
        assert!(Easing::EaseOut.apply(0.25) > Easing::Linear.apply(0.25));
    }
}
