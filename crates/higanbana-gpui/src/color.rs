//! Cores RGBA em float (alfa reto, não pré-multiplicado).
use higanbana_gpui_sys as sys;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// `rgb(0xRRGGBB)`, opaco.
pub const fn rgb(hex: u32) -> Color {
    rgba((hex << 8) | 0xff)
}

/// `rgba(0xRRGGBBAA)`.
pub const fn rgba(hex: u32) -> Color {
    Color {
        r: ((hex >> 24) & 0xff) as f32 / 255.0,
        g: ((hex >> 16) & 0xff) as f32 / 255.0,
        b: ((hex >> 8) & 0xff) as f32 / 255.0,
        a: (hex & 0xff) as f32 / 255.0,
    }
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    /// Mesma cor com alfa substituído.
    pub const fn alpha(self, a: f32) -> Color {
        Color { r: self.r, g: self.g, b: self.b, a }
    }

    /// Multiplica o alfa (usado para opacidade herdada).
    pub fn fade(self, factor: f32) -> Color {
        Color { a: self.a * factor, ..self }
    }

    pub fn is_transparent(&self) -> bool {
        self.a <= 0.0
    }

    /// Parse de `#rgb`, `#rrggbb` ou `#rrggbbaa`.
    pub fn from_hex(s: &str) -> Option<Color> {
        let h = s.strip_prefix('#').unwrap_or(s);
        let v = u32::from_str_radix(h, 16).ok()?;
        match h.len() {
            3 => {
                let x = |n: u32| ((v >> n) & 0xf) * 0x11;
                Some(rgb((x(8) << 16) | (x(4) << 8) | x(0)))
            }
            6 => Some(rgb(v)),
            8 => Some(rgba(v)),
            _ => None,
        }
    }

    pub(crate) fn to_hg(self) -> sys::hg_color {
        sys::hg_color { r: self.r, g: self.g, b: self.b, a: self.a }
    }
}

/// Converte RGBA8 de alfa reto para pré-multiplicado, no lugar (formato do core).
pub fn premultiply_rgba8(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(rgb(0xff0000), Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
        assert_eq!(rgba(0x00ff0080).a, 128.0 / 255.0);
        assert_eq!(Color::from_hex("#fff"), Some(Color::WHITE));
        assert_eq!(Color::from_hex("000000"), Some(Color::BLACK));
        assert_eq!(Color::from_hex("#12345"), None);
        assert_eq!(rgb(0x123456).alpha(0.5).a, 0.5);
    }

    #[test]
    fn premultiply() {
        let mut px = [255, 128, 0, 128, 10, 20, 30, 255, 200, 200, 200, 0];
        premultiply_rgba8(&mut px);
        assert_eq!(px, [128, 64, 0, 128, 10, 20, 30, 255, 0, 0, 0, 0]);
    }
}
