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
                let steps = crate::icon::raster::ROTATION_STEPS as f32;
                let step = (n.style.rotate.rem_euclid(1.0) * steps).round() as u32;
                *raster = crate::icon::raster::get_rotated(name, side.round().max(1.0) as u32, step);
            } else if let NodeKind::Polygon(points, _, raster) = &mut n.kind {
                let k = n.paint_scale * scale;
                let (w, h) = ((n.content.size.width * k).round() as u32, (n.content.size.height * k).round() as u32);
                *raster = crate::paint::shape::get(points, w, h);
            }
        }
    }
}
