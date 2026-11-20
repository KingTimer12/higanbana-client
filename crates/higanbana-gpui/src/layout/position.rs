//! Converte o layout relativo do taffy em retângulos absolutos, com rolagem e clips.
use super::engine::LayoutEngine;
use crate::element::ElementId;
use crate::geometry::{Rect, Size};
use crate::style::Overflow;
use crate::tree::node::{children, Node, NO_PARENT};
use std::collections::HashMap;

/// Deslocamentos de rolagem por id de elemento (sobrevivem às reconstruções).
pub(crate) type ScrollOffsets = HashMap<ElementId, f32>;

/// Preenche `rect`, `content`, `clip` e `scroll_max` de cada nó.
/// Também prende os deslocamentos guardados ao intervalo válido.
pub(crate) fn position<V>(engine: &LayoutEngine, nodes: &mut [Node<V>], viewport: Size, offsets: &mut ScrollOffsets) {
    for i in 0..nodes.len() {
        let l = engine.layout(i);
        let parent = nodes[i].parent;
        let (origin_x, origin_y, clip, scroll) = if parent == NO_PARENT {
            (0.0, 0.0, Rect::new(0.0, 0.0, viewport.width, viewport.height), 0.0)
        } else {
            let p = &nodes[parent as usize];
            let scroll = p.id.and_then(|id| offsets.get(&id).copied()).filter(|_| p.scroll_max > 0.0).unwrap_or(0.0);
            let clip = if p.style.overflow == Overflow::Visible { p.clip } else { p.clip.intersect(&padding_box(p)) };
            (p.rect.origin.x, p.rect.origin.y, clip, scroll)
        };
        let rect = Rect::new(origin_x + l.location.x, origin_y + l.location.y - scroll, l.size.width, l.size.height);
        let inset_l = l.border.left + l.padding.left;
        let inset_t = l.border.top + l.padding.top;
        let content = Rect::new(
            rect.origin.x + inset_l,
            rect.origin.y + inset_t,
            (rect.size.width - inset_l - l.border.right - l.padding.right).max(0.0),
            (rect.size.height - inset_t - l.border.bottom - l.padding.bottom).max(0.0),
        );
        let n = &mut nodes[i];
        n.rect = rect;
        n.content = content;
        n.clip = clip;
        if n.style.overflow == Overflow::Scroll {
            let max = scroll_extent(engine, nodes, i);
            nodes[i].scroll_max = max;
            if let Some(id) = nodes[i].id {
                if let Some(off) = offsets.get_mut(&id) {
                    *off = off.clamp(0.0, nodes[i].scroll_max);
                }
            }
        }
    }
}

/// Caixa de padding (o que um contêiner recortado mostra).
pub(crate) fn padding_box<V>(n: &Node<V>) -> Rect {
    let b = n.style.border_width;
    Rect::new(
        n.rect.origin.x + b,
        n.rect.origin.y + b,
        (n.rect.size.width - 2.0 * b).max(0.0),
        (n.rect.size.height - 2.0 * b).max(0.0),
    )
}

/// Quanto o conteúdo passa da altura visível do contêiner `i`.
fn scroll_extent<V>(engine: &LayoutEngine, nodes: &[Node<V>], i: usize) -> f32 {
    let l = engine.layout(i);
    let bottom = children(nodes, i)
        .map(|c| {
            let cl = engine.layout(c);
            cl.location.y + cl.size.height + cl.margin.bottom
        })
        .fold(0.0f32, f32::max);
    (bottom + l.padding.bottom + l.border.bottom - l.size.height).max(0.0)
}
