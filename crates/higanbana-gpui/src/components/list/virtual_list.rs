//! `VirtualList`: contêiner de rolagem com altura total fixa e só a faixa visível construída.
use crate::cx::RenderCx;
use crate::element::{div, scroll, Element, ElementId};
use crate::style::Styled;
use std::sync::Arc;

/// Faixa de itens a construir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub start: usize,
    /// Fim exclusivo.
    pub end: usize,
    /// y do primeiro item construído.
    pub top: f32,
    /// Altura de tudo (itens + espaços).
    pub total: f32,
}

/// y do item `index` (preso ao fim da lista).
pub fn item_offset(heights: &[f32], gap: f32, index: usize) -> f32 {
    let n = index.min(heights.len());
    heights[..n].iter().sum::<f32>() + gap * n as f32
}

/// Itens que cruzam `[offset - view, offset + 2·view]` (uma tela de folga para cada lado).
pub fn visible_range(heights: &[f32], gap: f32, offset: f32, view: f32) -> Range {
    let total = heights.iter().sum::<f32>() + gap * heights.len().saturating_sub(1) as f32;
    let (lo, hi) = (offset - view, offset + 2.0 * view);
    let (mut y, mut start, mut top) = (0.0, heights.len(), 0.0);
    let mut end = heights.len();
    for (i, h) in heights.iter().enumerate() {
        let bottom = y + h;
        if start == heights.len() && bottom > lo {
            start = i;
            top = y;
        }
        if y >= hi {
            end = i;
            break;
        }
        y = bottom + gap;
    }
    if start == heights.len() {
        return Range { start: 0, end: 0, top: 0.0, total };
    }
    Range { start, end: end.max(start), top, total }
}

pub struct VirtualList {
    id: ElementId,
    heights: Arc<[f32]>,
    gap: f32,
}

impl VirtualList {
    /// `heights`: altura de cada item (compartilhada, sem cópia por frame).
    pub fn new(id: impl Into<ElementId>, heights: Arc<[f32]>) -> Self {
        Self { id: id.into(), heights, gap: 0.0 }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Monta a lista chamando `item` só para os índices da faixa visível.
    /// Para ir a um item: `cx.scroll_to(id, item_offset(&heights, gap, i))`.
    pub fn render<V: 'static>(self, cx: &mut RenderCx<'_, V>, mut item: impl FnMut(usize, &mut RenderCx<'_, V>) -> Element<V>) -> Element<V> {
        let offset = cx.scroll_offset(self.id);
        let view = cx.scroll_view(self.id).unwrap_or(cx.viewport().height);
        let r = visible_range(&self.heights, self.gap, offset, view);
        let mut col = div().flex_col().flex_shrink_0().gap(self.gap).absolute().top(r.top).left(0.0).right(0.0);
        for i in r.start..r.end {
            col = col.child(item(i, cx).flex_shrink_0());
        }
        let body = div().flex_shrink_0().w_full().h(r.total).child(col);
        scroll(self.id).rerender_on_scroll().size_full().child(body)
    }
}
