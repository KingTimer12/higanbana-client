//! Barra de abas do kit (só os gatilhos; o conteúdo é do app). Variantes:
//! - `underline`: abas de 36 px sem fundo, espaço 16, linha de 1 px `border`
//!   embaixo da barra e 2 px `primary` sob a ativa;
//! - `pill`: abas de 32 px redondas, espaço 4, ativa em `primary`, hover `secondary`.
use super::common::{sub, sub_i, OnChange};
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Underline,
    Pill,
}

pub struct TabBar<V> {
    id: ElementId,
    active: usize,
    on_change: OnChange<V, usize>,
    labels: Vec<Cow<'static, str>>,
    look: Look,
}

impl<V: 'static> TabBar<V> {
    pub fn new(id: impl Into<ElementId>, active: usize, on_change: impl Fn(&mut V, usize, &mut EventCx) + 'static) -> Self {
        TabBar { id: id.into(), active, on_change: Rc::new(on_change), labels: Vec::new(), look: Look::Underline }
    }

    pub fn underline(mut self) -> Self {
        self.look = Look::Underline;
        self
    }

    pub fn pill(mut self) -> Self {
        self.look = Look::Pill;
        self
    }

    pub fn tab(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.labels.push(label.into());
        self
    }

    pub fn tab_id(id: impl Into<ElementId>, i: usize) -> ElementId {
        sub_i(id.into(), "aba", i)
    }
}

impl<V: 'static> Component<V> for TabBar<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = cx.theme();
        let underline = self.look == Look::Underline;
        let mut bar = div().id(self.id).relative().items_center().gap(if underline { 16.0 } else { 4.0 }).text_size(t.typography.sm).text_color(t.muted_foreground);
        if underline {
            bar = bar.child(div().id(sub(self.id, "linha")).absolute().left(0.0).right(0.0).bottom(0.0).h(1.0).bg(t.border));
        }
        for (i, label) in self.labels.into_iter().enumerate() {
            let on = i == self.active;
            let change = self.on_change.clone();
            let mut tab = div()
                .id(Self::tab_id(self.id, i))
                .relative()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .whitespace_nowrap()
                .on_click(move |v: &mut V, _, cx| {
                    change(v, i, cx);
                    cx.notify();
                })
                .child(text(label));
            tab = if underline {
                let fg = t.foreground;
                let tab = tab.h(36.0).pt(3.0).pb(4.0).hover(move |s| s.text_color(fg));
                if on {
                    tab.text_color(fg).child(div().absolute().left(0.0).right(0.0).bottom(0.0).h(2.0).bg(t.primary))
                } else {
                    tab
                }
            } else {
                let (sec, sec_fg) = (t.secondary, t.secondary_foreground);
                let tab = tab.h(32.0).px(12.0).rounded_full().text_color(t.foreground);
                if on { tab.bg(t.primary).text_color(t.primary_foreground) } else { tab.hover(move |s| s.bg(sec).text_color(sec_fg)) }
            };
            bar = bar.child(tab);
        }
        bar
    }
}
