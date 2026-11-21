//! `on_visible`: handlers dos elementos que entraram na área visível.
use super::Tree;
use crate::element::VisibleHandler;

impl<V> Tree<V> {
    /// Tira (consome) os handlers `on_visible` cujo elemento, ampliado pela margem,
    /// cruza a região visível herdada dos ancestrais e a janela.
    pub fn take_visible(&mut self) -> Vec<VisibleHandler<V>> {
        let view = crate::geometry::Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height);
        let mut out = Vec::new();
        for n in &mut self.nodes {
            let Some(x) = n.interact.as_deref_mut() else { continue };
            let Some((m, _)) = &x.on_visible else { continue };
            let r = n.rect;
            let grown = crate::geometry::Rect::new(r.origin.x - m, r.origin.y - m, r.size.width + 2.0 * m, r.size.height + 2.0 * m);
            if grown.intersects(&n.clip.intersect(&view)) {
                out.push(x.on_visible.take().expect("checado acima").1);
            }
        }
        out
    }
}
