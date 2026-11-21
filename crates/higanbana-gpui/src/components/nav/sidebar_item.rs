//! Item da Sidebar (kit `SidebarMenuItem`): ícone + rótulo; recolhido vira só
//! o ícone com dica à direita.
use crate::components::basic::Component;
use crate::components::overlay::{Tooltip, TooltipState};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId, MouseEvent, Side};
use crate::icon::icon;
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

type Click<V> = Rc<dyn Fn(&mut V, &MouseEvent, &mut EventCx)>;
pub(super) type TipLens<V> = Rc<dyn Fn(&mut V, ElementId) -> &mut TooltipState>;

/// `SidebarItem::new("animes", "Animes").icon("tv").active(true).on_click(..)`.
pub struct SidebarItem<V> {
    pub(super) id: ElementId,
    label: Cow<'static, str>,
    icon: Option<&'static str>,
    active: bool,
    on_click: Option<Click<V>>,
}

impl<V: 'static> SidebarItem<V> {
    pub fn new(id: impl Into<ElementId>, label: impl Into<Cow<'static, str>>) -> Self {
        SidebarItem { id: id.into(), label: label.into(), icon: None, active: false, on_click: None }
    }

    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }

    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut V, &MouseEvent, &mut EventCx) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// Monta o item; recolhido com ícone ganha dica (estado vem da Sidebar).
    pub(super) fn build(self, cx: &mut RenderCx<'_, V>, collapsed: bool, tip: (&TooltipState, TipLens<V>)) -> Element<V> {
        let t = cx.theme();
        let (accent, accent_fg) = (t.sidebar_accent, t.sidebar_accent_foreground);
        let mut e = div().id(self.id).items_center().gap(8.0).p(8.0).rounded(t.radius).text_size(t.typography.sm).cursor_pointer();
        if self.active {
            e = e.font_weight(500).bg(accent).text_color(accent_fg);
        } else {
            let hover = accent.alpha(accent.a * 0.8);
            e = e.hover(move |s| s.bg(hover).text_color(accent_fg));
        }
        if let Some(name) = self.icon {
            e = e.child(icon(name).size(16.0).flex_shrink_0());
        }
        e = if collapsed {
            e.justify_center()
        } else {
            let label = div().flex_1().min_w(0.0).overflow_hidden().whitespace_nowrap().child(text(self.label.clone()));
            e.w_full().h(28.0).child(label)
        };
        if let Some(f) = self.on_click {
            e = e.on_click(move |v, ev, cx| f(v, ev, cx));
        }
        if !(collapsed && self.icon.is_some()) {
            return e;
        }
        let (state, lens) = tip;
        let id = self.id;
        let tip_id = ElementId(id.0.rotate_left(11) ^ ElementId::from("dica").0);
        Tooltip::new(tip_id, state, move |v: &mut V| lens(v, id), e, self.label).side(Side::Right).render(cx)
    }
}
