//! Checkbox (shadcn): caixa 16 px com marca, estado indeterminado e rótulo clicável.
use super::style::{disabled_look, ring, ChangeFn};
use crate::color::Color;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::style::Styled;
use crate::theme::Theme;
use std::borrow::Cow;
use std::rc::Rc;

/// Estado da caixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckState {
    #[default]
    Unchecked,
    Checked,
    /// Parcial (ex.: "selecionar todos" com só alguns marcados).
    Indeterminate,
}

impl From<bool> for CheckState {
    fn from(b: bool) -> Self {
        if b { Self::Checked } else { Self::Unchecked }
    }
}

/// Caixa de seleção. Clique, Espaço/Enter ou clique no rótulo alternam;
/// `on_change` recebe o novo valor (indeterminado -> `true`).
pub struct Checkbox<'a, V> {
    id: ElementId,
    state: CheckState,
    theme: &'a Theme,
    ring_visible: bool,
    label: Option<Cow<'static, str>>,
    disabled: bool,
    on_change: Option<ChangeFn<V, bool>>,
}

impl<'a, V: 'static> Checkbox<'a, V> {
    pub fn new(cx: &RenderCx<'a, V>, id: impl Into<ElementId>, state: impl Into<CheckState>) -> Self {
        Self {
            id: id.into(),
            state: state.into(),
            theme: cx.theme(),
            ring_visible: cx.focus().visible,
            label: None,
            disabled: false,
            on_change: None,
        }
    }

    pub fn label(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&mut V, bool, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    fn mark(&self) -> Option<Element<V>> {
        let fg = self.theme.primary_foreground;
        match self.state {
            CheckState::Unchecked => None,
            CheckState::Checked => {
                Some(text("\u{2713}").font_family("Segoe UI Symbol").text_size(12.0).font_bold().text_color(fg))
            }
            CheckState::Indeterminate => Some(div().w(8.0).h(2.0).rounded(1.0).bg(fg)),
        }
    }

    fn build(self) -> Element<V> {
        let t = self.theme;
        let on = self.state != CheckState::Unchecked;
        let next = self.state != CheckState::Checked;
        let cb = self.on_change.clone();
        let toggle: Rc<dyn Fn(&mut V, &mut EventCx)> = Rc::new(move |v, cx| {
            if let Some(f) = &cb {
                f(v, next, cx);
            }
            cx.notify();
        });
        let idle_bg = if t.dark { t.input.alpha(0.3) } else { Color::TRANSPARENT };
        let t1 = toggle.clone();
        let mut bx = div()
            .track_focus(self.id)
            .size(16.0)
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(4.0)
            .border(1.0, if on { t.primary } else { t.input })
            .bg(if on { t.primary } else { idle_bg })
            .when(!self.disabled, |e| e.cursor_pointer())
            .when(self.ring_visible, |e| e.focus(ring(t.ring)))
            .on_click(move |v, _, cx| t1(v, cx));
        if let Some(m) = self.mark() {
            bx = bx.child(m);
        }
        let id = self.id;
        let row = div().flex_row().items_center().gap(8.0).child(bx).when_some(self.label, |row, l| {
            row.child(
                text(l)
                    .text_size(t.typography.sm)
                    .font_weight(500)
                    .text_color(t.foreground)
                    .when(!self.disabled, |e| e.cursor_pointer())
                    .on_click(move |v, _, cx| {
                        toggle(v, cx);
                        cx.focus(id);
                    }),
            )
        });
        disabled_look(row, self.disabled)
    }
}

/// `when` com valor opcional (só neste módulo, para não mexer em `Element`).
trait WhenSome: Sized {
    fn when_some<T>(self, v: Option<T>, f: impl FnOnce(Self, T) -> Self) -> Self {
        match v {
            Some(v) => f(self, v),
            None => self,
        }
    }
}

impl<V> WhenSome for Element<V> {}

impl<V: 'static> From<Checkbox<'_, V>> for Element<V> {
    fn from(c: Checkbox<'_, V>) -> Self {
        c.build()
    }
}
