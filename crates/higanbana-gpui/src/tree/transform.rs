//! translate/scale de subárvore aplicados depois do posicionamento: movem `rect`,
//! `content` e `clip` dos nós, então pintura e hit test já enxergam a posição visual.
use super::node::{Node, NO_PARENT};
use crate::layout::padding_box;
use crate::style::Overflow;
use super::Tree;
use crate::geometry::{Point, Rect};

/// Transformação afim só com escala uniforme e deslocamento: `p' = p * s + t`.
#[derive(Clone, Copy)]
struct Affine {
    s: f32,
    t: Point,
}

impl Affine {
    const ID: Affine = Affine { s: 1.0, t: Point { x: 0.0, y: 0.0 } };

    fn point(&self, p: Point) -> Point {
        Point { x: p.x * self.s + self.t.x, y: p.y * self.s + self.t.y }
    }

    fn rect(&self, r: Rect) -> Rect {
        let o = self.point(r.origin);
        Rect::new(o.x, o.y, r.size.width * self.s, r.size.height * self.s)
    }

    /// `self` depois de `inner` (inner aplicado primeiro).
    fn then(&self, inner: Affine) -> Affine {
        Affine { s: self.s * inner.s, t: self.point(inner.t) }
    }

    /// translate + scale do nó com centro `c` (já em coordenadas de layout).
    fn of_node(scale: f32, translate: Point, c: Point) -> Affine {
        Affine { s: scale, t: Point { x: c.x - c.x * scale + translate.x, y: c.y - c.y * scale + translate.y } }
    }
}

impl<V> Tree<V> {
    /// Aplica os transforms do estilo. Sem nenhum transform na árvore, não faz nada.
    pub(crate) fn apply_transforms(&mut self) {
        if !self.nodes.iter().any(|n| n.style.has_transform()) {
            return;
        }
        // Pilha de (fim da subárvore, transform acumulado) em pré-ordem.
        let mut stack: Vec<(usize, Affine)> = Vec::new();
        for i in 0..self.nodes.len() {
            while stack.last().is_some_and(|&(end, _)| i >= end) {
                stack.pop();
            }
            let parent = stack.last().map_or(Affine::ID, |&(_, a)| a);
            let n = &self.nodes[i];
            let mut acc = parent;
            if n.style.has_transform() {
                let c = n.rect.center();
                acc = parent.then(Affine::of_node(n.style.scale, n.style.translate, c));
            }
            let clip = match self.nodes[i].parent {
                NO_PARENT => self.nodes[i].clip,
                p => {
                    let p = &self.nodes[p as usize];
                    if p.style.overflow == Overflow::Visible { p.clip } else { p.clip.intersect(&padding_box(p)) }
                }
            };
            apply(&mut self.nodes[i], acc, clip);
            if acc.s != 1.0 || acc.t.x != 0.0 || acc.t.y != 0.0 {
                stack.push((self.nodes[i].end as usize, acc));
            }
        }
    }
}

/// Recorte recalculado a partir dos pais já transformados (mesma regra de `position`).
fn apply<V>(n: &mut Node<V>, acc: Affine, clip: Rect) {
    n.rect = acc.rect(n.rect);
    n.content = acc.rect(n.content);
    n.clip = clip;
    n.paint_scale = acc.s;
}
