//! Posição de overlays: âncora na janela, num ponto ou junto de um elemento,
//! virando para o lado oposto quando não cabe e presa dentro da janela.
use crate::element::{Anchor, AnchorAlign, Side};
use crate::geometry::{point, Point, Rect, Size};

/// Canto superior esquerdo do overlay de tamanho `size`.
/// `target` = caixa do elemento âncora (se `Anchor::Element` e ele existe).
pub fn place_overlay(anchor: &Anchor, gap: f32, size: Size, target: Option<Rect>, viewport: Size) -> Point {
    let p = match (anchor, target) {
        (Anchor::Window { x, y }, _) => point(
            gap + (viewport.width - size.width - 2.0 * gap) * x,
            gap + (viewport.height - size.height - 2.0 * gap) * y,
        ),
        (Anchor::Point(p), _) => *p,
        (Anchor::Element { side, align, .. }, Some(t)) => beside(t, *side, *align, gap, size, viewport),
        (Anchor::Element { .. }, None) => point((viewport.width - size.width) / 2.0, (viewport.height - size.height) / 2.0),
    };
    let x = p.x.min(viewport.width - size.width).max(0.0);
    let y = p.y.min(viewport.height - size.height).max(0.0);
    point(x, y)
}

fn cross(start: f32, len: f32, size: f32, align: AnchorAlign) -> f32 {
    match align {
        AnchorAlign::Start => start,
        AnchorAlign::Center => start + (len - size) / 2.0,
        AnchorAlign::End => start + len - size,
    }
}

fn beside(t: Rect, side: Side, align: AnchorAlign, gap: f32, size: Size, vp: Size) -> Point {
    let below = t.bottom() + gap;
    let above = t.origin.y - gap - size.height;
    let right = t.right() + gap;
    let left = t.origin.x - gap - size.width;
    // Vira se não cabe e o lado oposto tem mais espaço.
    let side = match side {
        Side::Bottom if below + size.height > vp.height && t.origin.y > vp.height - t.bottom() => Side::Top,
        Side::Top if above < 0.0 && vp.height - t.bottom() > t.origin.y => Side::Bottom,
        Side::Right if right + size.width > vp.width && t.origin.x > vp.width - t.right() => Side::Left,
        Side::Left if left < 0.0 && vp.width - t.right() > t.origin.x => Side::Right,
        s => s,
    };
    match side {
        Side::Bottom => point(cross(t.origin.x, t.size.width, size.width, align), below),
        Side::Top => point(cross(t.origin.x, t.size.width, size.width, align), above),
        Side::Right => point(right, cross(t.origin.y, t.size.height, size.height, align)),
        Side::Left => point(left, cross(t.origin.y, t.size.height, size.height, align)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::size;

    const VP: Size = size(400.0, 300.0);

    #[test]
    fn centered_in_window() {
        assert_eq!(place_overlay(&Anchor::center(), 0.0, size(100.0, 50.0), None, VP), point(150.0, 125.0));
        assert_eq!(place_overlay(&Anchor::window(1.0, 1.0), 8.0, size(100.0, 50.0), None, VP), point(292.0, 242.0));
    }

    #[test]
    fn below_flips_above_when_no_room() {
        let a = Anchor::below("t");
        let top = Rect::new(10.0, 10.0, 80.0, 20.0);
        assert_eq!(place_overlay(&a, 4.0, size(80.0, 100.0), Some(top), VP), point(10.0, 34.0));
        let bottom = Rect::new(10.0, 260.0, 80.0, 20.0);
        assert_eq!(place_overlay(&a, 4.0, size(80.0, 100.0), Some(bottom), VP), point(10.0, 156.0));
    }

    #[test]
    fn right_flips_left_and_clamps() {
        let a = Anchor::right_of("t").align(AnchorAlign::Center);
        let t = Rect::new(350.0, 100.0, 40.0, 20.0);
        let p = place_overlay(&a, 4.0, size(60.0, 30.0), Some(t), VP);
        assert_eq!(p, point(286.0, 95.0));
        let huge = place_overlay(&Anchor::at(point(390.0, 290.0)), 0.0, size(60.0, 30.0), None, VP);
        assert_eq!(huge, point(340.0, 270.0), "preso dentro da janela");
    }
}
