//! Sidebar (kit): coluna com cabeçalho, grupos rolando e rodapé; recolhe para
//! 48 px (só ícones) animando a largura em 200 ms.
use super::sidebar_item::{SidebarItem, TipLens};
use crate::anim::{Easing, Transition};
use crate::components::basic::{Button, Component};
use crate::components::overlay::TooltipState;
use crate::components::{ComponentSize, Variant};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, scroll, text, Element, ElementId, MouseEvent};
use crate::icon::icon;
use crate::style::Styled;
use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

/// Largura aberta do kit.
pub const SIDEBAR_WIDTH: f32 = 255.0;
/// Largura recolhida (só ícones).
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 48.0;
const RESIZE: Transition = Transition::ms(200).easing(Easing::EaseInOut);

/// Estado na view: recolhida e as dicas dos itens (modo recolhido).
#[derive(Debug, Default)]
pub struct SidebarState {
    pub collapsed: bool,
    tips: HashMap<ElementId, TooltipState>,
}

/// Grupo com rótulo (some quando recolhida).
pub struct SidebarGroup<V> {
    label: Cow<'static, str>,
    items: Vec<SidebarItem<V>>,
}

impl<V: 'static> SidebarGroup<V> {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        SidebarGroup { label: label.into(), items: Vec::new() }
    }

    pub fn item(mut self, item: SidebarItem<V>) -> Self {
        self.items.push(item);
        self
    }
}

type StateLens<V> = Rc<dyn Fn(&mut V) -> &mut SidebarState>;

/// `Sidebar::new("nav", &v.side, |v| &mut v.side).header(..).group(..).footer(..)`.
pub struct Sidebar<'a, V> {
    id: ElementId,
    state: &'a SidebarState,
    lens: StateLens<V>,
    header: Option<Element<V>>,
    footer: Option<Element<V>>,
    groups: Vec<SidebarGroup<V>>,
}

impl<'a, V: 'static> Sidebar<'a, V> {
    pub fn new(id: impl Into<ElementId>, state: &'a SidebarState, lens: impl Fn(&mut V) -> &mut SidebarState + 'static) -> Self {
        Sidebar { id: id.into(), state, lens: Rc::new(lens), header: None, footer: None, groups: Vec::new() }
    }

    pub fn header(mut self, e: impl Into<Element<V>>) -> Self {
        self.header = Some(e.into());
        self
    }

    pub fn footer(mut self, e: impl Into<Element<V>>) -> Self {
        self.footer = Some(e.into());
        self
    }

    pub fn group(mut self, g: SidebarGroup<V>) -> Self {
        self.groups.push(g);
        self
    }
}

fn part(id: ElementId, name: &str) -> ElementId {
    ElementId(id.0.rotate_left(13) ^ ElementId::from(name).0)
}

impl<V: 'static> Component<V> for Sidebar<'_, V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let collapsed = self.state.collapsed;
        let target = if collapsed { SIDEBAR_COLLAPSED_WIDTH } else { SIDEBAR_WIDTH };
        let w = cx.animate(part(self.id, "largura"), target, RESIZE);
        let pad = if collapsed { 8.0 } else { 12.0 };
        let label_fg = {
            let f = cx.theme().sidebar_foreground;
            f.alpha(f.a * 0.7)
        };
        let xs = cx.theme().typography.xs;
        // Kit: grupos colados (o `gap_y_3` dele fica num contêiner de um filho só).
        let mut body = scroll(part(self.id, "grupos")).flex_1().flex_col().px(pad);
        let n = self.groups.len();
        for (i, g) in self.groups.into_iter().enumerate() {
            let mut col = div().flex_col().w_full();
            if i == 0 {
                col = col.pt(pad);
            }
            if i + 1 == n {
                col = col.pb(pad);
            }
            if !collapsed {
                col = col.child(div().flex_shrink_0().items_center().h(32.0).px(8.0).text_size(xs).text_color(label_fg).child(text(g.label)));
            }
            let mut items = div().flex_col().w_full().gap(8.0);
            for it in g.items {
                let lens = self.lens.clone();
                let tl: TipLens<V> = Rc::new(move |v: &mut V, k| lens(v).tips.entry(k).or_default());
                let st = self.state.tips.get(&it.id).copied().unwrap_or_default();
                items = items.child(it.build(cx, collapsed, (&st, tl)));
            }
            body = body.child(col.child(items));
        }
        let mut col = div().flex_col().h_full().flex_1().min_w(0.0).overflow_hidden();
        if let Some(h) = self.header {
            col = col.child(div().flex_shrink_0().flex_col().pt(if collapsed { 8.0 } else { 12.0 }).px(pad).gap(8.0).child(h));
        }
        col = col.child(body);
        if let Some(f) = self.footer {
            col = col.child(div().flex_shrink_0().flex_col().pb(12.0).pt(if collapsed { 8.0 } else { 0.0 }).px(pad).gap(8.0).child(f));
        }
        let t = cx.theme();
        // Borda direita de 1 px (a lib só tem borda nos quatro lados).
        div()
            .id(self.id)
            .flex_shrink_0()
            .h_full()
            .w(w)
            .bg(t.sidebar)
            .text_color(t.sidebar_foreground)
            .child(col)
            .child(div().w(1.0).h_full().flex_shrink_0().bg(t.sidebar_border))
    }
}

/// Botão de recolher/expandir (ghost, small, painel aberto/fechado).
pub struct SidebarToggle<V> {
    id: ElementId,
    collapsed: bool,
    on_click: Option<Rc<dyn Fn(&mut V, &MouseEvent, &mut EventCx)>>,
}

impl<V: 'static> SidebarToggle<V> {
    pub fn new(id: impl Into<ElementId>, collapsed: bool) -> Self {
        SidebarToggle { id: id.into(), collapsed, on_click: None }
    }

    pub fn on_click(mut self, f: impl Fn(&mut V, &MouseEvent, &mut EventCx) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl<V: 'static> Component<V> for SidebarToggle<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let name = if self.collapsed { "panel-left-open" } else { "panel-left-close" };
        let mut b = Button::new(self.id, "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon(name).size(16.0));
        if let Some(f) = self.on_click {
            b = b.on_click(move |v, ev, cx| f(v, ev, cx));
        }
        b.render(cx)
    }
}
