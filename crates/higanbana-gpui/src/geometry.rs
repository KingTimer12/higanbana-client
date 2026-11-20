//! Tipos geométricos em pixels lógicos.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

/// Valores por lado (top, right, bottom, left).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

/// Valores por canto, na ordem do core: tl, tr, br, bl.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners<T> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_right: T,
    pub bottom_left: T,
}

pub const fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

pub const fn size(width: f32, height: f32) -> Size {
    Size { width, height }
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { origin: Point { x, y }, size: Size { width, height } }
    }

    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Borda esquerda/superior inclusiva, direita/inferior exclusiva.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.origin.x && p.y >= self.origin.y && p.x < self.right() && p.y < self.bottom()
    }

    /// Interseção; tamanho zero quando não se tocam.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.origin.x.max(other.origin.x);
        let y = self.origin.y.max(other.origin.y);
        let w = (self.right().min(other.right()) - x).max(0.0);
        let h = (self.bottom().min(other.bottom()) - y).max(0.0);
        Rect::new(x, y, w, h)
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        !self.intersect(other).is_empty()
    }

    /// Menor retângulo que contém os dois.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.origin.x.min(other.origin.x);
        let y = self.origin.y.min(other.origin.y);
        Rect::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }

    pub fn is_empty(&self) -> bool {
        self.size.width <= 0.0 || self.size.height <= 0.0
    }

    /// Multiplica tudo por `s` (lógico -> físico).
    pub fn scale(&self, s: f32) -> Rect {
        Rect::new(self.origin.x * s, self.origin.y * s, self.size.width * s, self.size.height * s)
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.origin.x + dx, self.origin.y + dy, self.size.width, self.size.height)
    }
}

impl<T: Copy> Edges<T> {
    pub const fn all(v: T) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
}

impl<T: Copy> Corners<T> {
    pub const fn all(v: T) -> Self {
        Self { top_left: v, top_right: v, bottom_right: v, bottom_left: v }
    }

    pub fn to_array(&self) -> [T; 4] {
        [self.top_left, self.top_right, self.bottom_right, self.bottom_left]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_half_open() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert!(r.contains(point(10.0, 10.0)));
        assert!(!r.contains(point(30.0, 15.0)));
        assert!(!r.contains(point(9.9, 15.0)));
    }

    #[test]
    fn intersect_union_scale() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(&b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert!(a.intersect(&Rect::new(20.0, 0.0, 1.0, 1.0)).is_empty());
        assert_eq!(a.scale(2.0), Rect::new(0.0, 0.0, 20.0, 20.0));
        assert_eq!(Corners::all(3.0).to_array(), [3.0; 4]);
    }
}
