//! Hit testing sobre o último layout (de cima para baixo).
use crate::geometry::Point;
use crate::tree::node::{Node, NO_PARENT};
use crate::tree::Tree;

impl<V> Tree<V> {
    /// Nó mais acima sob `p`. Pré-ordem invertida = ordem de pintura invertida,
    /// então irmãos posteriores e descendentes ganham.
    pub fn hit_test(&self, p: Point) -> Option<usize> {
        (0..self.nodes.len()).rev().find(|&i| {
            let n = &self.nodes[i];
            n.clip.contains(p) && n.rect.contains(p)
        })
    }

    /// Primeiro de `i` e seus ancestrais que satisfaz `pred`.
    pub(crate) fn nearest(&self, i: usize, pred: impl Fn(&Node<V>) -> bool) -> Option<usize> {
        let mut cur = i as u32;
        while cur != NO_PARENT {
            let n = &self.nodes[cur as usize];
            if pred(n) {
                return Some(cur as usize);
            }
            cur = n.parent;
        }
        None
    }

    /// Elemento com estilo de hover mais profundo que contém o nó sob o mouse.
    pub(crate) fn hover_owner(&self, hovered: Option<u32>) -> Option<usize> {
        hovered.and_then(|h| self.nearest(h as usize, |n| n.hover.is_some()))
    }

    /// O nó `i` está sob o mouse (ele ou um descendente)?
    pub fn is_hovered(&self, i: usize) -> bool {
        self.hovered.is_some_and(|h| crate::tree::node::contains(&self.nodes, i, h as usize))
    }
}
