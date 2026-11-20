//! Switch (shadcn): trilho 32x18 com polegar que desliza (150 ms, ease-out) e
//! cor do trilho animada junto.
use super::style::{disabled_look, mix, ring, ChangeFn};
use crate::anim::{Easing, Transition};
use crate::color::Color;
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, Element, ElementId};
use crate::style::{px, Styled};
use std::rc::Rc;

const TRACK_W: f32 = 32.0;
const TRACK_H: f32 = 18.0;
const THUMB: f32 = 16.0;
/// Curso do polegar dentro do trilho (descontado 1 px de borda de cada lado).
const TRAVEL: f32 = TRACK_W - 2.0 - THUMB;

/// Liga/desliga. Clique, Espaço ou Enter alternam; `on_change` recebe o novo valor.
pub struct Switch<V> {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeFn<V, bool>>,
}

impl<V: 'static> Switch<V> {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self { id: id.into(), checked, disabled: false, on_change: None }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&mut V, bool, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl<V: 'static> Component<V> for Switch<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let target = if self.checked { 1.0 } else { 0.0 };
        // Posição animada 0 (desligado) .. 1 (ligado).
        let p = cx.animate(self.id, target, Transition::ms(150).easing(Easing::EaseOut));
        let th = cx.theme();
        let off_track = if th.dark { th.input.alpha(0.8) } else { th.input };
        let (thumb_off, thumb_on) = if th.dark { (th.foreground, th.primary_foreground) } else { (th.background, th.background) };
        let next = !self.checked;
        let cb = self.on_change;
        let thumb = div().size(THUMB).flex_shrink_0().rounded_full().bg(mix(thumb_off, thumb_on, p)).ml(px(p * TRAVEL));
        let track = div()
            .track_focus(self.id)
            .w(TRACK_W)
            .h(TRACK_H)
            .flex_shrink_0()
            .flex_row()
            .items_center()
            .rounded_full()
            .border(1.0, Color::TRANSPARENT)
            .bg(mix(off_track, th.primary, p))
            .cursor_pointer()
            .when(cx.focus().visible, |e| e.focus(ring(th.ring)))
            .on_click(move |v, _, cx| {
                if let Some(f) = &cb {
                    f(v, next, cx);
                }
                cx.notify();
            })
            .child(thumb);
        disabled_look(track, self.disabled)
    }
}
