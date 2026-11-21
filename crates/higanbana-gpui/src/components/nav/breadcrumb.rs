//! Breadcrumb do kit: itens em `muted_foreground` separados por chevron-right
//! (14 px); o último em `foreground` e sem clique.
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::icon::icon;
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

type Go<V> = Rc<dyn Fn(&mut V, &mut EventCx)>;

pub struct Breadcrumb<V> {
    id: ElementId,
    items: Vec<(Cow<'static, str>, Option<Go<V>>)>,
}

impl<V: 'static> Breadcrumb<V> {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Breadcrumb { id: id.into(), items: Vec::new() }
    }

    /// Item que navega ao clicar (o último nunca navega).
    pub fn link(mut self, label: impl Into<Cow<'static, str>>, go: impl Fn(&mut V, &mut EventCx) + 'static) -> Self {
        self.items.push((label.into(), Some(Rc::new(go))));
        self
    }

    /// Item só de texto.
    pub fn item(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.items.push((label.into(), None));
        self
    }
}

impl<V: 'static> Component<V> for Breadcrumb<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = cx.theme();
        let (muted, fg) = (t.muted_foreground, t.foreground);
        let n = self.items.len();
        let mut row = div().id(self.id).flex_wrap().items_center().gap(6.0).text_size(t.typography.sm).text_color(muted);
        for (i, (label, go)) in self.items.into_iter().enumerate() {
            if i > 0 {
                row = row.child(icon("chevron-right").size(14.0).text_color(muted));
            }
            let last = i + 1 == n;
            let mut item = div().id(ElementId(ElementId::from(("migalha", i)).0 ^ self.id.0.rotate_left(17))).child(text(label));
            if last {
                item = item.text_color(fg);
            } else if let Some(go) = go {
                item = item.cursor_pointer().hover(move |s| s.text_color(fg)).on_click(move |v: &mut V, _, cx| {
                    go(v, cx);
                    cx.notify();
                });
            }
            row = row.child(item);
        }
        row
    }
}
