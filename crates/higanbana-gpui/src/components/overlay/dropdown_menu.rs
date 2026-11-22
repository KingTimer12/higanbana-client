//! Menu suspenso do kit (`dropdown_menu` + `PopupMenuItem`): painel com p 4,
//! itens de 26 px (px 8, texto sm), ✓ à esquerda no marcado, hover `accent`.
//! Fecha ao escolher, com Esc ou clique fora (via `Popover`).
use super::common::{sub_i, Lens};
use super::popover::{Popover, PopoverAnchor, PopoverState};
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, scroll, text, Element, ElementId};
use crate::icon::icon;
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

type Pick<V> = Rc<dyn Fn(&mut V, &mut EventCx)>;

pub struct DropdownMenu<V> {
    id: ElementId,
    state: PopoverState,
    lens: Lens<V, PopoverState>,
    trigger: Element<V>,
    anchor: PopoverAnchor,
    items: Vec<(Cow<'static, str>, bool, Pick<V>)>,
    max_height: Option<f32>,
}

impl<V: 'static> DropdownMenu<V> {
    pub fn new(id: impl Into<ElementId>, state: &PopoverState, lens: impl Fn(&mut V) -> &mut PopoverState + 'static, trigger: Element<V>) -> Self {
        DropdownMenu { id: id.into(), state: *state, lens: Rc::new(lens), trigger, anchor: PopoverAnchor::TopLeft, items: Vec::new(), max_height: None }
    }

    pub fn anchor(mut self, a: PopoverAnchor) -> Self {
        self.anchor = a;
        self
    }

    /// Item; `checked` mostra o ✓.
    pub fn item(mut self, label: impl Into<Cow<'static, str>>, checked: bool, pick: impl Fn(&mut V, &mut EventCx) + 'static) -> Self {
        self.items.push((label.into(), checked, Rc::new(pick)));
        self
    }

    /// Rola a lista acima desta altura.
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }

    pub fn item_id(id: impl Into<ElementId>, i: usize) -> ElementId {
        sub_i(id.into(), "item", i)
    }

    pub fn check_id(id: impl Into<ElementId>, i: usize) -> ElementId {
        sub_i(id.into(), "marca", i)
    }
}

impl<V: 'static> Component<V> for DropdownMenu<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let id = self.id;
        let has_check = self.items.iter().any(|i| i.1);
        let (items, lens, max_h) = (self.items, self.lens.clone(), self.max_height);
        let close = self.lens.clone();
        Popover::new(id, &self.state, move |v: &mut V| lens(v), self.trigger)
            .anchor(self.anchor)
            .padding(4.0)
            .content(move |cx| {
                let t = cx.theme();
                let (accent, accent_fg) = (t.accent, t.accent_foreground);
                let mut list = match max_h {
                    Some(h) => scroll(sub_i(id, "lista", 0)).max_h(h),
                    None => div().flex_col(),
                };
                list = list.gap(2.0).min_w(128.0).max_w(500.0);
                for (i, (label, checked, pick)) in items.into_iter().enumerate() {
                    let close = close.clone();
                    let mut row = div()
                        .id(Self::item_id(id, i))
                        .h(26.0)
                        .px(8.0)
                        .gap(4.0)
                        .items_center()
                        .rounded(t.radius)
                        .cursor_pointer()
                        .hover(move |s| s.bg(accent).text_color(accent_fg))
                        .on_click(move |v: &mut V, _, cx| {
                            close(v).open = false;
                            pick(v, cx);
                            cx.notify();
                        });
                    if has_check {
                        let mark = div().w(12.0).h(12.0).flex_shrink_0();
                        row = row.child(if checked { mark.id(Self::check_id(id, i)).child(icon("check").size(12.0)) } else { mark });
                    }
                    list = list.child(row.child(text(label)));
                }
                list
            })
            .render(cx)
    }
}
