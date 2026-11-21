//! Mola amortecida (massa-mola) em forma fechada: valor e velocidade em qualquer instante,
//! sem integrar quadro a quadro.

/// Parâmetros físicos da mola. `Spring::DEFAULT` = "gentle" (170 / 26 / 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

/// Uma mola nunca anima mais que isso: depois, encaixa no alvo (sem frames eternos).
pub const MAX_SPRING_SECS: f32 = 10.0;

impl Spring {
    pub const DEFAULT: Spring = Spring { stiffness: 170.0, damping: 26.0, mass: 1.0 };

    pub const fn new(stiffness: f32, damping: f32) -> Self {
        Spring { stiffness, damping, mass: 1.0 }
    }

    /// Valor e velocidade em `t` segundos, saindo de `from` com velocidade `v0` rumo a `to`.
    pub fn sample(&self, from: f32, to: f32, v0: f32, t: f32) -> (f32, f32) {
        let (x, v) = self.displacement(from - to, v0, t.max(0.0));
        (to + x, v)
    }

    /// A mola parou (perto do alvo e quase sem velocidade)?
    pub fn settled(value: f32, velocity: f32, from: f32, to: f32) -> bool {
        (value - to).abs() < 1e-3 * (to - from).abs().max(1.0) && velocity.abs() < 1e-2
    }

    /// Parâmetros inválidos (NaN, ≤ 0) caem no padrão; amortecimento ganha um mínimo.
    fn sane(&self) -> (f32, f32, f32) {
        let ok = |v: f32, min: f32, def: f32| if v.is_finite() && v > 0.0 { v.max(min) } else { def };
        (ok(self.stiffness, 1.0, Self::DEFAULT.stiffness), ok(self.damping, 0.5, Self::DEFAULT.damping), ok(self.mass, 0.01, 1.0))
    }

    fn displacement(&self, x0: f32, v0: f32, t: f32) -> (f32, f32) {
        let (k, c, m) = self.sane();
        let w0 = (k / m).sqrt();
        let z = c / (2.0 * (k * m).sqrt());
        if (z - 1.0).abs() < 1e-4 {
            let b = v0 + w0 * x0;
            let e = (-w0 * t).exp();
            let x = (x0 + b * t) * e;
            (x, e * (b - w0 * (x0 + b * t)))
        } else if z < 1.0 {
            let a = z * w0;
            let wd = w0 * (1.0 - z * z).sqrt();
            let b = (v0 + a * x0) / wd;
            let (s, c) = (wd * t).sin_cos();
            let e = (-a * t).exp();
            let x = e * (x0 * c + b * s);
            (x, e * (-a * (x0 * c + b * s) + (-x0 * wd * s + b * wd * c)))
        } else {
            let r = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w0 * (z - r), -w0 * (z + r));
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            let c2 = x0 - c1;
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
        }
    }
}

impl Default for Spring {
    fn default() -> Self {
        Spring::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_at_target() {
        for s in [Spring::DEFAULT, Spring::new(100.0, 20.0), Spring::new(100.0, 40.0), Spring::new(300.0, 5.0)] {
            let (x, v) = s.sample(0.0, 1.0, 0.0, 6.0);
            assert!((x - 1.0).abs() < 1e-3, "{s:?}: {x}");
            assert!(v.abs() < 1e-2, "{s:?}: {v}");
        }
    }

    #[test]
    fn critically_damped_no_overshoot() {
        let k: f32 = 200.0;
        let s = Spring::new(k, 2.0 * k.sqrt());
        for i in 0..200 {
            let (x, _) = s.sample(0.0, 1.0, 0.0, i as f32 * 0.01);
            assert!(x <= 1.0 + 1e-4, "passou do alvo em {i}: {x}");
        }
    }

    #[test]
    fn starts_at_from_with_v0() {
        let (x, v) = Spring::DEFAULT.sample(3.0, 1.0, 5.0, 0.0);
        assert!((x - 3.0).abs() < 1e-5);
        assert!((v - 5.0).abs() < 1e-4);
    }
}

#[cfg(test)]
mod degenerate {
    use crate::anim::{Animator, Transition};
    use crate::element::ElementId;
    use std::time::{Duration, Instant};

    /// Molas sem amortecimento, NaN ou rigidez 0 não podem animar para sempre.
    #[test]
    fn degenerate_springs_settle() {
        for tr in [Transition::spring(170.0, 0.0), Transition::spring(170.0, f32::NAN), Transition::spring(0.0, 26.0), Transition::spring(f32::NAN, 26.0)] {
            let (t0, k) = (Instant::now(), ElementId::from("m"));
            let mut a = Animator::default();
            a.begin();
            a.value(k, 0.0, 1.0, tr, t0);
            a.end();
            a.begin();
            let v = a.value(k, 0.0, 1.0, tr, t0 + Duration::from_secs(11));
            assert!(!a.is_active(), "{tr:?} ainda ativa");
            assert_eq!(v, 1.0);
        }
    }
}
