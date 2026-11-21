//! Curvas de suavização e a descrição de uma transição.
use super::repeat::Repeat;
use super::spring::Spring;
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

/// Duração + curva, ou mola. `Transition::default()` = 150 ms, ease-out (padrão shadcn).
/// Com `spring` definido, `duration`/`easing` são ignorados: a mola dita o tempo.
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub duration: Duration,
    pub easing: Easing,
    pub spring: Option<Spring>,
    /// Espera antes de começar.
    pub delay: Duration,
    /// Repetições (só transições com duração; molas ignoram).
    pub repeat: Repeat,
    /// Repetições alternam ida e volta.
    pub yoyo: bool,
}

impl Transition {
    pub const fn new(duration: Duration, easing: Easing) -> Self {
        Self { duration, easing, spring: None, delay: Duration::ZERO, repeat: Repeat::Once, yoyo: false }
    }

    pub const fn ms(ms: u64) -> Self {
        Self { duration: Duration::from_millis(ms), easing: Easing::EaseOut, spring: None, delay: Duration::ZERO, repeat: Repeat::Once, yoyo: false }
    }

    /// Mola com rigidez e amortecimento (massa 1). Mudar o alvo preserva a velocidade.
    pub const fn spring(stiffness: f32, damping: f32) -> Self {
        Self::with_spring(Spring::new(stiffness, damping))
    }

    pub const fn with_spring(spring: Spring) -> Self {
        Self { duration: Duration::ZERO, easing: Easing::Linear, spring: Some(spring), delay: Duration::ZERO, repeat: Repeat::Once, yoyo: false }
    }

    pub const fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub const fn delay(mut self, ms: u64) -> Self {
        self.delay = Duration::from_millis(ms);
        self
    }

    pub const fn repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }

    pub const fn yoyo(mut self) -> Self {
        self.yoyo = true;
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
