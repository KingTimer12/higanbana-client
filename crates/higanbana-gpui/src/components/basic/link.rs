//! Link do kit: texto na cor `link` com sublinhado meio transparente; no hover
//! clareia e o sublinhado fica cheio. O que o clique faz (abrir URL) é do app.
use super::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

pub struct Link<V> {
    id: ElementId,
    label: Cow<'static, str>,
    on_click: Option<Rc<dyn Fn(&mut V, &mut EventCx)>>,
}

impl<V: 'static> Link<V> {
    pub fn new(id: impl Into<ElementId>, label: impl Into<Cow<'static, str>>) -> Self {
        Link { id: id.into(), label: label.into(), on_click: None }
    }

    pub fn on_click(mut self, f: impl Fn(&mut V, &mut EventCx) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl<V: 'static> Component<V> for Link<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let link = cx.theme().link;
        let mut e = div()
            .id(self.id)
            .flex_shrink_0()
            .relative()
            .flex_col()
            .text_color(link)
            .cursor_pointer()
            .hover(move |s| s.text_color(link.alpha(0.8)))
            .child(text(self.label))
            // Sublinhado: o core não tem borda só embaixo.
            .child(div().absolute().left(0.0).right(0.0).bottom(0.0).h(1.0).bg(link.alpha(0.5)));
        if let Some(f) = self.on_click {
            e = e.on_click(move |v: &mut V, _, cx| {
                f(v, cx);
                cx.notify();
            });
        }
        e
    }
}
