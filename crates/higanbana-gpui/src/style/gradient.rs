//! Gradiente linear de fundo (scrims sobre vídeo/imagens).
use crate::color::Color;

/// Para onde o gradiente vai: `Down` = primeira parada no topo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradientDir {
    Down,
    Up,
    Right,
    Left,
}

/// Paradas `(posição 0..1, cor)` em ordem crescente de posição.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    pub dir: GradientDir,
    pub stops: Vec<(f32, Color)>,
}

impl LinearGradient {
    pub fn new(dir: GradientDir, stops: Vec<(f32, Color)>) -> Self {
        LinearGradient { dir, stops }
    }

    /// Duas cores, de `from` (início) a `to` (fim).
    pub fn scrim(dir: GradientDir, from: Color, to: Color) -> Self {
        LinearGradient { dir, stops: vec![(0.0, from), (1.0, to)] }
    }

    /// Cor em `t` (0..1) interpolada entre as paradas.
    pub fn sample(&self, t: f32) -> Color {
        let Some(&(p0, c0)) = self.stops.first() else { return Color::TRANSPARENT };
        if t <= p0 {
            return c0;
        }
        for w in self.stops.windows(2) {
            let ((a, ca), (b, cb)) = (w[0], w[1]);
            if t <= b {
                let k = if b > a { (t - a) / (b - a) } else { 1.0 };
                return Color { r: ca.r + (cb.r - ca.r) * k, g: ca.g + (cb.g - ca.g) * k, b: ca.b + (cb.b - ca.b) * k, a: ca.a + (cb.a - ca.a) * k };
            }
        }
        self.stops.last().map_or(c0, |s| s.1)
    }
}
