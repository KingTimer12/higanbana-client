//! Tipos animáveis: viram até 4 componentes `f32` interpolados um a um.
use crate::color::Color;
use crate::geometry::{Point, Rect, Size};

/// Valor que o `Animator` sabe interpolar componente a componente.
pub trait Animatable: Copy {
    fn to_arr(self) -> [f32; 4];
    fn from_arr(a: [f32; 4]) -> Self;
}

impl Animatable for f32 {
    fn to_arr(self) -> [f32; 4] {
        [self, 0.0, 0.0, 0.0]
    }
    fn from_arr(a: [f32; 4]) -> Self {
        a[0]
    }
}

impl Animatable for Color {
    fn to_arr(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    fn from_arr([r, g, b, a]: [f32; 4]) -> Self {
        Color { r, g, b, a }
    }
}

impl Animatable for Point {
    fn to_arr(self) -> [f32; 4] {
        [self.x, self.y, 0.0, 0.0]
    }
    fn from_arr(a: [f32; 4]) -> Self {
        Point { x: a[0], y: a[1] }
    }
}

impl Animatable for Size {
    fn to_arr(self) -> [f32; 4] {
        [self.width, self.height, 0.0, 0.0]
    }
    fn from_arr(a: [f32; 4]) -> Self {
        Size { width: a[0], height: a[1] }
    }
}

impl Animatable for Rect {
    fn to_arr(self) -> [f32; 4] {
        [self.origin.x, self.origin.y, self.size.width, self.size.height]
    }
    fn from_arr(a: [f32; 4]) -> Self {
        Rect::new(a[0], a[1], a[2], a[3])
    }
}

#[cfg(test)]
mod tests {
    use crate::anim::{Animator, Easing, Transition};
    use crate::color::Color;
    use crate::element::ElementId;
    use crate::geometry::Rect;
    use std::time::{Duration, Instant};

    const LINEAR: Transition = Transition::ms(100).easing(Easing::Linear);

    #[test]
    fn color_interpolates_per_channel() {
        let t0 = Instant::now();
        let k = ElementId::from("cor");
        let (black, white) = (Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }, Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
        let mut a = Animator::default();
        a.begin();
        a.animate(k, black, white, LINEAR, t0);
        let c = a.animate(k, black, white, LINEAR, t0 + Duration::from_millis(50));
        assert!((c.r - 0.5).abs() < 1e-3 && (c.g - 0.5).abs() < 1e-3 && (c.b - 0.5).abs() < 1e-3);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn rect_animates() {
        let t0 = Instant::now();
        let k = ElementId::from("ret");
        let mut a = Animator::default();
        a.begin();
        let r = a.animate(k, Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(100.0, 0.0, 10.0, 10.0), LINEAR, t0);
        assert_eq!(r.origin.x, 0.0);
        let r = a.animate(k, Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(100.0, 0.0, 10.0, 10.0), LINEAR, t0 + Duration::from_millis(50));
        assert!((r.origin.x - 50.0).abs() < 1e-3 && r.size.width == 10.0);
    }
}
