//! Matemática do Slider: fração, passo e teclas.
use crate::input::Key;

/// Faixa `[min, max]` com passo (`step <= 0` = contínuo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Range {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl Range {
    /// Posição 0..1 de `v` na faixa.
    pub fn fraction(&self, v: f32) -> f32 {
        if self.max <= self.min { 0.0 } else { ((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0) }
    }

    /// Limita à faixa e arredonda para o passo mais próximo.
    pub fn snap(&self, v: f32) -> f32 {
        let v = v.clamp(self.min, self.max);
        if self.step <= 0.0 {
            return v;
        }
        let n = ((v - self.min) / self.step).round();
        (self.min + n * self.step).min(self.max)
    }

    /// Valor na fração `f` (clique/arraste no trilho).
    pub fn at_fraction(&self, f: f32) -> f32 {
        self.snap(self.min + f.clamp(0.0, 1.0) * (self.max - self.min))
    }

    /// Setas = 1 passo, PageUp/PageDown = 10 passos, Home/End = extremos.
    pub fn key(&self, key: Key, v: f32) -> Option<f32> {
        let s = if self.step > 0.0 { self.step } else { (self.max - self.min) / 100.0 };
        let n = match key {
            Key::Left | Key::Down => v - s,
            Key::Right | Key::Up => v + s,
            Key::PageDown => v - 10.0 * s,
            Key::PageUp => v + 10.0 * s,
            Key::Home => self.min,
            Key::End => self.max,
            _ => return None,
        };
        Some(self.snap(n))
    }
}
