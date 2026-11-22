//! Slider (shadcn): trilho, faixa preenchida e polegar; arraste com captura do
//! ponteiro, clique no trilho salta e teclado ajusta.
use super::slider_value::Range;
use super::style::{disabled_look, ring, ChangeFn};
use crate::color::Color;
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, Element, ElementId};
use crate::style::{pct, px, Length, Styled};
use std::rc::Rc;

const THUMB: f32 = 16.0;

/// Valor em `[min, max]` com passo. `on_change` só é chamado quando o valor muda.
pub struct Slider<V> {
    kit: bool,
    id: ElementId,
    value: f32,
    range: Range,
    width: Length,
    disabled: bool,
    on_change: Option<ChangeFn<V, f32>>,
    on_release: Option<ChangeFn<V, f32>>,
}

impl<V: 'static> Slider<V> {
    /// Faixa padrão 0..100, passo 1.
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            kit: false,
            id: id.into(),
            value,
            range: Range { min: 0.0, max: 100.0, step: 1.0 },
            width: pct(100.0),
            disabled: false,
            on_change: None,
            on_release: None,
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

    /// Visual do kit: polegar branco sem borda (ex.: volume do player).
    pub fn kit(mut self) -> Self {
        self.kit = true;
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

    /// Valor final ao soltar o botão (ex.: pular na faixa só no fim do arraste).
    pub fn on_release(mut self, f: impl Fn(&mut V, f32, &mut EventCx) + 'static) -> Self {
        self.on_release = Some(Rc::new(f));
        self
    }
}

impl<V: 'static> Component<V> for Slider<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let (t, r, value, id) = (cx.theme(), self.range, self.value, self.id);
        let cb = self.on_change;
        let emit: Rc<dyn Fn(&mut V, f32, &mut EventCx)> = Rc::new(move |v, n, cx| {
            if n != value
                && let Some(f) = &cb {
                    f(v, n, cx);
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
            .bg(if self.kit { Color::WHITE } else { t.background })
            .border(if self.kit { 0.0 } else { 1.0 }, t.primary)
            .shadow(Color::BLACK.alpha(0.1), 2.0)
            .when(cx.focus().visible, |e| e.focus(ring(t.ring)));
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
            .when_some(self.on_release, move |e, f| {
                e.on_mouse_up(move |v, ev, cx| {
                    f(v, r.at_fraction(ev.fraction_x()), cx);
                    cx.notify();
                })
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
