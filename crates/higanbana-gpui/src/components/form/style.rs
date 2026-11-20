//! Peças visuais comuns do grupo `form`: anel de foco, desabilitado e mistura de cores.
use crate::color::Color;
use crate::cx::EventCx;
use crate::element::Element;
use crate::gfx::CursorStyle;
use crate::style::{Style, Styled};
use std::rc::Rc;

/// Callback de mudança compartilhado entre vários handlers do mesmo componente.
pub(super) type ChangeFn<V, T> = Rc<dyn Fn(&mut V, T, &mut EventCx)>;

/// Anel de foco do shadcn (`border-ring` + `ring-[3px] ring-ring/50`).
pub(super) fn ring(c: Color) -> impl Fn(Style) -> Style + 'static {
    move |s| s.border_color(c).shadow(c.alpha(0.5), 3.0)
}

/// Desabilitado: 50% de opacidade, cursor proibido e nenhum handler.
pub(super) fn disabled_look<V>(e: Element<V>, disabled: bool) -> Element<V> {
    if !disabled {
        return e;
    }
    e.cursor(CursorStyle::NotAllowed).disabled(true).disabled_style(|s| s.opacity(0.5))
}

/// Interpola `a` -> `b` (t em 0..1), usado nas transições de cor.
pub(super) fn mix(a: Color, b: Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Color { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: l(a.a, b.a) }
}
