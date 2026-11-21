//! Carousel: fileira de itens com deslize animado até o item atual, setas
//! anterior/próximo (botão outline redondo, fora da área dos itens, como o kit)
//! e auto-avanço opcional pelo relógio.
mod state;
#[cfg(test)]
mod tests;

pub use state::CarouselState;

use std::rc::Rc;

use crate::anim::{Easing, Transition};
use crate::components::{Button, Component, ComponentSize, Variant};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, Element, ElementId};
use crate::icon::icon;
use crate::style::Styled;

/// Duração do deslize (BASE do app, ease-out cúbico).
const SLIDE: Transition = Transition::new(std::time::Duration::from_millis(240), Easing::EaseOut);
/// Distância entre a seta e a área dos itens (`mr_4`/`ml_4` do kit).
const ARROW_GAP: f32 = 16.0;

type Lens<V> = Rc<dyn Fn(&mut V) -> &mut CarouselState>;

/// Deslocamento para alinhar o item `index` à esquerda, preso ao fim da fileira.
/// `item_w = None`: cada item ocupa a janela inteira (banner).
pub fn target_offset(index: usize, len: usize, item_w: Option<f32>, gap: f32, view_w: f32) -> f32 {
    let w = item_w.unwrap_or(view_w);
    let total = len as f32 * w + len.saturating_sub(1) as f32 * gap;
    (index as f32 * (w + gap)).min((total - view_w).max(0.0))
}

fn sub(id: ElementId, part: &str) -> ElementId {
    ElementId(id.0.rotate_left(17) ^ ElementId::from(part).0)
}

pub struct Carousel<V> {
    id: ElementId,
    state: CarouselState,
    lens: Lens<V>,
    items: Vec<Element<V>>,
    item_w: Option<f32>,
    gap: f32,
    controls: bool,
    on_change: Option<Rc<dyn Fn(&mut V, usize, &mut EventCx)>>,
}

impl<V: 'static> Carousel<V> {
    /// `state` é uma cópia do estado atual; `lens` aponta para ele na view.
    pub fn new(id: impl Into<ElementId>, state: &CarouselState, lens: impl Fn(&mut V) -> &mut CarouselState + 'static) -> Self {
        Self { id: id.into(), state: state.clone(), lens: Rc::new(lens), items: Vec::new(), item_w: None, gap: 0.0, controls: false, on_change: None }
    }

    pub fn items(mut self, items: Vec<Element<V>>) -> Self {
        self.items = items;
        self
    }

    /// Largura fixa de cada item (fileiras de cartões). Sem isto, um item por vez.
    pub fn item_width(mut self, w: f32) -> Self {
        self.item_w = Some(w);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Setas anterior/próximo fora da área dos itens (reserve espaço dos lados).
    pub fn controls(mut self, on: bool) -> Self {
        self.controls = on;
        self
    }

    /// Chamado quando uma seta troca o item.
    pub fn on_change(mut self, f: impl Fn(&mut V, usize, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    fn arrow(&self, cx: &mut RenderCx<'_, V>, next: bool, enabled: bool) -> Element<V> {
        let (lens, change) = (self.lens.clone(), self.on_change.clone());
        let id = sub(self.id, if next { "proximo" } else { "anterior" });
        let b = Button::new(id, "")
            .variant(Variant::Outline)
            .size(ComponentSize::Compact)
            .icon(icon(if next { "chevron-right" } else { "chevron-left" }).size(14.0))
            .disabled(!enabled)
            .on_click(move |v: &mut V, _, cx| {
                let now = std::time::Instant::now();
                let s = lens(v);
                if if next { s.next(now) } else { s.prev(now) } {
                    let i = s.current(now);
                    if let Some(f) = &change {
                        f(v, i, cx);
                    }
                }
                cx.notify();
            })
            .render(cx)
            .rounded_full();
        let side = ComponentSize::Compact.height() + ARROW_GAP;
        let place = div().absolute().top(0.0).bottom(0.0).items_center();
        if next { place.right(-side) } else { place.left(-side) }.child(b)
    }

    pub fn render(mut self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let now = cx.now();
        let len = self.items.len();
        let s = &self.state;
        if let Some(at) = s.next_change(now) {
            cx.request_redraw_in(at.saturating_duration_since(now));
        }
        let index = s.current(now).min(len.saturating_sub(1));
        let view_w = cx.bounds(self.id).map_or(0.0, |r| r.size.width);
        let target = target_offset(index, len, self.item_w, self.gap, view_w);
        let x = cx.animate(sub(self.id, "x"), target, SLIDE);
        let mut row = div().flex_shrink_0().gap(self.gap).ml(-x);
        let (has_prev, has_next) = (s.has_prev(now), s.has_next(now));
        let w = self.item_w.unwrap_or(view_w);
        for it in std::mem::take(&mut self.items) {
            row = row.child(div().flex_shrink_0().w(w).child(it));
        }
        let viewport = div().id(self.id).track_bounds().w_full().overflow_hidden().child(row);
        if !self.controls {
            return viewport;
        }
        let (prev, next) = (self.arrow(cx, false, has_prev), self.arrow(cx, true, has_next));
        div().relative().w_full().child(viewport).child(prev).child(next)
    }
}
