//! Converte o layout relativo do taffy em retângulos absolutos, com rolagem e clips.
use super::engine::LayoutEngine;
use crate::element::ElementId;
use crate::geometry::{Point, Rect, Size};
use crate::style::Overflow;
use crate::tree::node::{children, Node, NO_PARENT};
use std::collections::HashMap;
use std::ops::Range;

/// Deslocamentos de rolagem por id de elemento (sobrevivem às reconstruções).
pub(crate) type ScrollOffsets = HashMap<ElementId, f32>;

/// Preenche `rect`, `content`, `clip` e `scroll_max` dos nós em `range` (uma raiz e
/// sua subárvore). A raiz fica em `origin`. Também prende os deslocamentos guardados.
pub(crate) fn position<V>(
    engine: &LayoutEngine,
    nodes: &mut [Node<V>],
    range: Range<usize>,
    origin: Point,
    viewport: Size,
    offsets: &mut ScrollOffsets,
) {
    for i in range {
        let l = engine.layout(i);
        let parent = nodes[i].parent;
        let (origin_x, origin_y, clip, scroll) = if parent == NO_PARENT {
            (origin.x, origin.y, Rect::new(0.0, 0.0, viewport.width, viewport.height), 0.0)
        } else {
            let p = &nodes[parent as usize];
            let scroll = p.id.and_then(|id| offsets.get(&id).copied()).filter(|_| p.scroll_max > 0.0).unwrap_or(0.0);
            let clip = if p.style.overflow == Overflow::Visible { p.clip } else { p.clip.intersect(&padding_box(p)) };
            (p.rect.origin.x, p.rect.origin.y, clip, scroll)
        };
        let horizontal = parent != NO_PARENT && nodes[parent as usize].style.overflow == Overflow::ScrollX;
        let (sx, sy) = if horizontal { (scroll, 0.0) } else { (0.0, scroll) };
        let rect = Rect::new(origin_x + l.location.x - sx, origin_y + l.location.y - sy, l.size.width, l.size.height);
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
        if n.style.overflow.is_scroll() {
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

/// Quanto o conteúdo passa do tamanho visível do contêiner `i` (no eixo que rola).
fn scroll_extent<V>(engine: &LayoutEngine, nodes: &[Node<V>], i: usize) -> f32 {
    let l = engine.layout(i);
    if nodes[i].style.overflow == Overflow::ScrollX {
        let right = children(nodes, i)
            .map(|c| {
                let cl = engine.layout(c);
                cl.location.x + cl.size.width + cl.margin.right
            })
            .fold(0.0f32, f32::max);
        return (right + l.padding.right + l.border.right - l.size.width).max(0.0);
    }
    let bottom = children(nodes, i)
        .map(|c| {
            let cl = engine.layout(c);
            cl.location.y + cl.size.height + cl.margin.bottom
        })
        .fold(0.0f32, f32::max);
    (bottom + l.padding.bottom + l.border.bottom - l.size.height).max(0.0)
}
