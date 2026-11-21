//! RadioGroup (shadcn): círculos de 16 px, escolha única, setas movem a escolha.
use super::style::{ring, ChangeFn};
use crate::color::Color;
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::input::Key;
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

/// `RadioGroup::new("sub", sel).items(["Desligada", "pt"]).on_change(|v, i, cx| ..)`.
pub struct RadioGroup<V> {
    id: ElementId,
    selected: Option<usize>,
    items: Vec<Cow<'static, str>>,
    horizontal: bool,
    kit: bool,
    on_change: Option<ChangeFn<V, usize>>,
}

impl<V: 'static> RadioGroup<V> {
    pub fn new(id: impl Into<ElementId>, selected: Option<usize>) -> Self {
        RadioGroup { id: id.into(), selected, items: Vec::new(), horizontal: false, kit: false, on_change: None }
    }

    pub fn item(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.items.push(label.into());
        self
    }

    pub fn items<S: Into<Cow<'static, str>>>(mut self, labels: impl IntoIterator<Item = S>) -> Self {
        self.items.extend(labels.into_iter().map(Into::into));
        self
    }

    /// Itens lado a lado (padrão: empilhados).
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    /// Visual do kit: marcado = círculo cheio com ✓ (menus do player).
    pub fn kit(mut self) -> Self {
        self.kit = true;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&mut V, usize, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Id da linha do item `i`.
    pub fn item_id(id: impl Into<ElementId>, i: usize) -> ElementId {
        let id = id.into();
        ElementId(id.0.rotate_left(11) ^ ElementId::from(("radio", i)).0)
    }
}

/// Círculo: marcado = primária cheia com miolo claro.
fn dot<V: 'static>(on: bool, kit: bool, t: &crate::theme::Theme) -> Element<V> {
    let idle = if t.dark { t.input.alpha(0.3) } else { Color::TRANSPARENT };
    div()
        .size(16.0)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded_full()
        .border(1.0, if on { t.primary } else { t.input })
        .bg(if on { t.primary } else { idle })
        .when(on && !kit, |e| e.child(div().size(6.0).rounded_full().bg(t.primary_foreground)))
        .when(on && kit, |e| e.child(crate::icon::icon("check").size(12.0).text_color(t.primary_foreground)))
}

impl<V: 'static> Component<V> for RadioGroup<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = cx.theme();
        let (id, n, sel, kit) = (self.id, self.items.len(), self.selected, self.kit);
        let change = self.on_change;
        let pick: Rc<dyn Fn(&mut V, usize, &mut EventCx)> = Rc::new(move |v, i, cx| {
            if let Some(f) = &change {
                f(v, i, cx);
            }
            cx.notify();
        });
        let keys = pick.clone();
        let mut group = div()
            .track_focus(id)
            .gap(if self.horizontal { 16.0 } else { 12.0 })
            .when(!self.horizontal, |e| e.flex_col())
            .when(self.horizontal, |e| e.flex_row().flex_wrap())
            .when(cx.focus().visible, |e| e.focus(ring(t.ring)))
            .on_key_down(move |v: &mut V, ev, cx| {
                let cur = sel.unwrap_or(0);
                let next = match ev.key {
                    Key::Down | Key::Right => (cur + 1).min(n.saturating_sub(1)),
                    Key::Up | Key::Left => cur.saturating_sub(1),
                    _ => return,
                };
                if n > 0 && Some(next) != sel {
                    keys(v, next, cx);
                }
                cx.stop_propagation();
            });
        for (i, label) in self.items.into_iter().enumerate() {
            let p = pick.clone();
            group = group.child(
                div()
                    .id(Self::item_id(id, i))
                    .flex_row()
                    .items_center()
                    .gap(8.0)
                    .cursor_pointer()
                    .text_size(t.typography.sm)
                    .on_click(move |v: &mut V, _, cx| {
                        p(v, i, cx);
                        cx.focus(id);
                    })
                    .child(dot(sel == Some(i), kit, t))
                    .child(text(label)),
            );
        }
        group
    }
}
