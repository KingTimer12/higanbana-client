//! SVG → máscara RGBA (branco com alfa) no tamanho físico; a cor vem do tint no desenho.
use crate::element::{ImageData, ImageSource};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// (nome, lado em px físicos) → imagem. Poucos ícones × poucos tamanhos: sem despejo.
    static CACHE: RefCell<HashMap<(String, u32), Option<ImageSource>>> = RefCell::new(HashMap::new());
}

/// Ícone `name` com `px`×`px` pixels físicos; `None` se o nome não existe ou o SVG é inválido.
pub fn get(name: &str, px: u32) -> Option<ImageSource> {
    let px = px.clamp(1, 1024);
    let key = (name.to_string(), px);
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let made = super::registry::svg(name).and_then(|svg| rasterize(svg, px));
    CACHE.with(|c| c.borrow_mut().insert(key, made.clone()));
    made
}

pub(crate) fn clear() {
    CACHE.with(|c| c.borrow_mut().clear());
}

fn rasterize(svg: &[u8], px: u32) -> Option<ImageSource> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pix = resvg::tiny_skia::Pixmap::new(px, px)?;
    let size = tree.size();
    let s = px as f32 / size.width().max(size.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pix.as_mut());
    // Só a cobertura importa: branco + alfa, tingido no desenho.
    let mut out = pix.take();
    for p in out.chunks_exact_mut(4) {
        let a = p[3];
        p.copy_from_slice(&[255, 255, 255, a]);
    }
    Some(ImageData::from_rgba8(px, px, out))
}
