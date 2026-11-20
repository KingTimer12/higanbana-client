//! Switch (shadcn): trilho 32x18 com polegar que desliza (150 ms, ease-out) e
//! cor do trilho animada junto.
use super::style::{disabled_look, mix, ring, ChangeFn};
use crate::anim::{Easing, Transition};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, Element, ElementId};
use crate::style::{px, Styled};
use crate::theme::Theme;
use std::rc::Rc;

const TRACK_W: f32 = 32.0;
const TRACK_H: f32 = 18.0;
const THUMB: f32 = 16.0;
/// Curso do polegar dentro do trilho (descontado 1 px de borda de cada lado).
const TRAVEL: f32 = TRACK_W - 2.0 - THUMB;

/// Liga/desliga. Clique, Espaço ou Enter alternam; `on_change` recebe o novo valor.
pub struct Switch<'a, V> {
    id: ElementId,
    checked: bool,
    /// Posição animada 0 (desligado) .. 1 (ligado).
    t: f32,
    theme: &'a Theme,
    ring_visible: bool,
    disabled: bool,
    on_change: Option<ChangeFn<V, bool>>,
}

impl<'a, V: 'static> Switch<'a, V> {
    /// Consulta a animação já aqui (por isso `&mut RenderCx`).
    pub fn new(cx: &mut RenderCx<'a, V>, id: impl Into<ElementId>, checked: bool) -> Self {
        let id = id.into();
        let target = if checked { 1.0 } else { 0.0 };
        let t = cx.animate(id, target, Transition::ms(150).easing(Easing::EaseOut));
        Self { id, checked, t, theme: cx.theme(), ring_visible: cx.focus().visible, disabled: false, on_change: None }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&mut V, bool, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    fn build(self) -> Element<V> {
        let th = self.theme;
        let off_track = if th.dark { th.input.alpha(0.8) } else { th.input };
        let (thumb_off, thumb_on) = if th.dark { (th.foreground, th.primary_foreground) } else { (th.background, th.background) };
        let next = !self.checked;
        let cb = self.on_change;
        let thumb = div().size(THUMB).flex_shrink_0().rounded_full().bg(mix(thumb_off, thumb_on, self.t)).ml(px(self.t * TRAVEL));
        let track = div()
            .track_focus(self.id)
            .w(TRACK_W)
            .h(TRACK_H)
            .flex_shrink_0()
            .flex_row()
            .items_center()
            .rounded_full()
            .border(1.0, crate::color::Color::TRANSPARENT)
            .bg(mix(off_track, th.primary, self.t))
            .when(!self.disabled, |e| e.cursor_pointer())
            .when(self.ring_visible, |e| e.focus(ring(th.ring)))
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

impl<V: 'static> From<Switch<'_, V>> for Element<V> {
    fn from(s: Switch<'_, V>) -> Self {
        s.build()
    }
}
