//! Tabs (shadcn): lista de gatilhos em fundo `muted`; o ativo ganha fundo e sombra.
//! Setas esquerda/direita (e Home/End) movem o foco e ativam a aba.
use crate::components::basic::Component;
use super::common::{ring, sub, sub_i, OnChange};
use crate::color::rgba;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::input::Key;
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

/// Abas com conteúdo por índice. Só o conteúdo ativo entra na árvore.
pub struct Tabs<V> {
    id: ElementId,
    active: usize,
    on_change: OnChange<V, usize>,
    tabs: Vec<(Cow<'static, str>, Element<V>)>,
}

impl<V: 'static> Tabs<V> {
    /// `active` vem da view; `on_change` recebe o novo índice.
    pub fn new(id: impl Into<ElementId>, active: usize, on_change: impl Fn(&mut V, usize, &mut EventCx) + 'static) -> Self {
        Self { id: id.into(), active, on_change: Rc::new(on_change), tabs: Vec::new() }
    }

    /// Acrescenta uma aba (rótulo do gatilho + conteúdo).
    pub fn tab(mut self, label: impl Into<Cow<'static, str>>, content: impl Into<Element<V>>) -> Self {
        self.tabs.push((label.into(), content.into()));
        self
    }

    /// Id do gatilho `i` (para focar programaticamente).
    pub fn trigger_id(id: impl Into<ElementId>, i: usize) -> ElementId {
        sub_i(id.into(), "aba", i)
    }

}

impl<V: 'static> Component<V> for Tabs<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let theme = cx.theme();
        let ring_on = cx.focus().visible;
        let (id, n, active) = (self.id, self.tabs.len(), self.active.min(self.tabs.len().saturating_sub(1)));
        let keys = self.on_change.clone();
        let mut list = div()
            .id(sub(id, "lista"))
            .h(36.0)
            .p(3.0)
            .items_center()
            .rounded(theme.radius)
            .bg(theme.muted)
            .text_color(theme.muted_foreground)
            .text_size(theme.typography.sm)
            .on_key_down(move |v: &mut V, ev, cx| {
                let next = match ev.key {
                    Key::Left => (active + n - 1) % n.max(1),
                    Key::Right => (active + 1) % n.max(1),
                    Key::Home => 0,
                    Key::End => n.saturating_sub(1),
                    _ => return,
                };
                keys(v, next, cx);
                cx.focus(Self::trigger_id(id, next));
                cx.notify();
                cx.stop_propagation();
            });
        let mut content = None;
        for (i, (label, body)) in self.tabs.into_iter().enumerate() {
            let change = self.on_change.clone();
            let mut trig = div()
                .track_focus(Self::trigger_id(id, i))
                .h_full()
                .px(12.0)
                .items_center()
                .justify_center()
                .rounded(theme.radius - 2.0)
                .font_weight(500)
                .cursor_pointer()
                .on_click(move |v: &mut V, _, cx| {
                    change(v, i, cx);
                    cx.notify();
                })
                .child(text(label));
            if i == active {
                trig = trig.bg(theme.background).text_color(theme.foreground).shadow(rgba(0x0000_0026), 3.0);
                content = Some(body);
            }
            list = list.child(ring(trig, theme, ring_on));
        }
        let root = div().id(id).flex_col().gap(8.0).child(list);
        match content {
            Some(c) => root.child(div().id(sub(id, "conteudo")).child(c)),
            None => root,
        }
    }
}
