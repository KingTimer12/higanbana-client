//! Slider (shadcn): trilho, faixa preenchida e polegar; arraste com captura do
//! ponteiro, clique no trilho salta e teclado ajusta.
use super::slider_value::Range;
use super::style::{disabled_look, ring, ChangeFn};
use crate::color::Color;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, Element, ElementId};
use crate::style::{pct, px, Length, Styled};
use crate::theme::Theme;
use std::rc::Rc;

const THUMB: f32 = 16.0;

/// Valor em `[min, max]` com passo. `on_change` só é chamado quando o valor muda.
pub struct Slider<'a, V> {
    id: ElementId,
    value: f32,
    range: Range,
    width: Length,
    theme: &'a Theme,
    ring_visible: bool,
    disabled: bool,
    on_change: Option<ChangeFn<V, f32>>,
}

impl<'a, V: 'static> Slider<'a, V> {
    /// Faixa padrão 0..100, passo 1.
    pub fn new(cx: &RenderCx<'a, V>, id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            range: Range { min: 0.0, max: 100.0, step: 1.0 },
            width: pct(100.0),
            theme: cx.theme(),
            ring_visible: cx.focus().visible,
            disabled: false,
            on_change: None,
        }
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.range.min = min;
        self.range.max = max;
        self
    }

    /// Passo (`0` = contínuo).
    pub fn step(mut self, step: f32) -> Self {
        self.range.step = step;
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        self.width = w.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&mut V, f32, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    fn build(self) -> Element<V> {
        let (t, r, value, id) = (self.theme, self.range, self.value, self.id);
        let cb = self.on_change;
        let emit: Rc<dyn Fn(&mut V, f32, &mut EventCx)> = Rc::new(move |v, n, cx| {
            if n != value {
                if let Some(f) = &cb {
                    f(v, n, cx);
                }
            }
            cx.notify();
        });
        let (e1, e2, e3) = (emit.clone(), emit.clone(), emit);
        let frac = r.fraction(value) * 100.0;
        let range = div().h_full().w(pct(frac)).bg(t.primary);
        let track = div().w_full().h(6.0).rounded_full().bg(t.muted).overflow_hidden().child(range);
        let thumb = div()
            .track_focus(id)
            .absolute()
            .top(0.0)
            .left(pct(frac))
            .ml(px(-THUMB / 2.0))
            .size(THUMB)
            .rounded_full()
            .bg(t.background)
            .border(1.0, t.primary)
            .shadow(Color::BLACK.alpha(0.1), 2.0)
            .when(self.ring_visible, |e| e.focus(ring(t.ring)));
        let root = div()
            .relative()
            .w(self.width)
            .h(THUMB)
            .flex_row()
            .items_center()
            .when(!self.disabled, |e| e.cursor_pointer())
            .on_mouse_down(move |v, ev, cx| {
                e1(v, r.at_fraction(ev.fraction_x()), cx);
                cx.focus(id);
            })
            .on_mouse_move(move |v, ev, cx| {
                if ev.pressed {
                    e2(v, r.at_fraction(ev.fraction_x()), cx);
                }
            })
            .on_key_down(move |v, ev, cx| {
                if let Some(n) = r.key(ev.key, value) {
                    e3(v, n, cx);
                    cx.stop_propagation();
                }
            })
            .child(track)
            .child(thumb);
        disabled_look(root, self.disabled)
    }
}

impl<V: 'static> From<Slider<'_, V>> for Element<V> {
    fn from(s: Slider<'_, V>) -> Self {
        s.build()
    }
}
