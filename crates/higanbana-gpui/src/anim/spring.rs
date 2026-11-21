//! Mola amortecida (massa-mola) em forma fechada: valor e velocidade em qualquer instante,
//! sem integrar quadro a quadro.

/// Parâmetros físicos da mola. `Spring::DEFAULT` = "gentle" (170 / 26 / 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

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

    fn displacement(&self, x0: f32, v0: f32, t: f32) -> (f32, f32) {
        let m = self.mass.max(1e-4);
        let w0 = (self.stiffness.max(1e-4) / m).sqrt();
        let z = self.damping.max(0.0) / (2.0 * (self.stiffness.max(1e-4) * m).sqrt());
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
