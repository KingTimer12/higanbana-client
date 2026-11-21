//! Regiões de hit test da janela (barra customizada) tiradas da árvore.
use super::Tree;
use crate::geometry::Rect;
use crate::gfx::{HitKind, HitRegion};
use crate::style::Display;

impl<V> Tree<V> {
    /// Regiões em ordem de pintura (a última vence no core):
    /// - nó com `window_hit` → a região dele;
    /// - nó clicável/interativo dentro de uma área de arrastar → `Client`;
    /// - cada overlay aberto → `Client` (tela toda se modal).
    pub fn hit_regions(&self, out: &mut Vec<HitRegion>) {
        out.clear();
        let end = self.main_end();
        // Fim da subárvore de arrastar mais externa ativa.
        let mut drag_end = 0usize;
        let mut hidden_end = 0usize;
        for i in 0..end {
            let n = &self.nodes[i];
            if i < hidden_end {
                continue;
            }
            if n.style.display == Display::None {
                hidden_end = n.end as usize;
                continue;
            }
            let visible = n.rect.intersect(&n.clip);
            if visible.is_empty() {
                continue;
            }
            match n.style.window_hit {
                Some(kind) => {
                    if kind == HitKind::Caption && i >= drag_end {
                        drag_end = n.end as usize;
                    }
                    out.push(HitRegion { rect: visible, kind });
                }
                None if i < drag_end && (n.on_click.is_some() || n.interact.is_some()) => {
                    out.push(HitRegion { rect: visible, kind: HitKind::Client });
                }
                None => {}
            }
        }
        if out.is_empty() {
            return;
        }
        for o in &self.overlays {
            let rect = if o.spec.modal {
                Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height)
            } else {
                self.nodes[o.root as usize].rect
            };
            out.push(HitRegion { rect, kind: HitKind::Client });
        }
    }
}
