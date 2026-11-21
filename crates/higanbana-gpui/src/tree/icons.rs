//! Rasterização dos ícones da árvore para a escala da janela.
use super::node::NodeKind;
use super::Tree;

impl<V> Tree<V> {
    /// Liga cada nó de ícone à imagem no tamanho físico (`lado lógico * scale`).
    /// Chamar depois do layout e antes de `ImageCache::prepare`.
    pub fn resolve_icons(&mut self, scale: f32) {
        for n in &mut self.nodes {
            if let NodeKind::Icon(name, raster) = &mut n.kind {
                let side = n.content.size.width.min(n.content.size.height) * n.paint_scale * scale;
                *raster = crate::icon::raster::get(name, side.round().max(1.0) as u32);
            }
        }
    }
}
