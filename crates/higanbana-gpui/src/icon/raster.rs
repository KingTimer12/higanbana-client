//! SVG → máscara RGBA (branco com alfa) no tamanho físico; a cor vem do tint no desenho.
use crate::element::{ImageData, ImageSource};
use crate::paint::cache::{self, AgedMap};
use std::cell::RefCell;

thread_local! {
    /// (nome, lado em px físicos, passo de rotação) → imagem. Poucos ícones × tamanhos: sem despejo.
    static CACHE: RefCell<AgedMap<(String, u32, u32)>> = RefCell::new(AgedMap::default());
}

/// Passos de rotação por volta (ícones girando, ex.: Spinner): 7.5° cada.
pub const ROTATION_STEPS: u32 = 48;

/// Ícone `name` com `px`×`px` pixels físicos; `None` se o nome não existe ou o SVG é inválido.
pub fn get(name: &str, px: u32) -> Option<ImageSource> {
    get_rotated(name, px, 0)
}

/// Como `get`, girado `step / ROTATION_STEPS` de volta (horário) em torno do centro.
pub fn get_rotated(name: &str, px: u32, step: u32) -> Option<ImageSource> {
    let px = px.clamp(1, 1024);
    let step = step % ROTATION_STEPS;
    let key = (name.to_string(), px, step);
    if let Some(hit) = CACHE.with(|c| c.borrow_mut().get(&key)) {
        return hit;
    }
    cache::register(sweep);
    let made = super::registry::svg(name).and_then(|svg| rasterize(svg, px, step));
    CACHE.with(|c| c.borrow_mut().insert(key, made.clone()));
    made
}

fn sweep(now: u64) {
    CACHE.with(|c| c.borrow_mut().sweep(now));
}

pub(crate) fn clear() {
    CACHE.with(|c| c.borrow_mut().clear());
}

fn rasterize(svg: &[u8], px: u32, step: u32) -> Option<ImageSource> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pix = resvg::tiny_skia::Pixmap::new(px, px)?;
    let size = tree.size();
    let s = px as f32 / size.width().max(size.height());
    let deg = step as f32 * 360.0 / ROTATION_STEPS as f32;
    let c = px as f32 / 2.0;
    let t = resvg::tiny_skia::Transform::from_scale(s, s).post_rotate_at(deg, c, c);
    resvg::render(&tree, t, &mut pix.as_mut());
    // Só a cobertura importa: branco + alfa, tingido no desenho.
    let mut out = pix.take();
    for p in out.as_chunks_mut::<4>().0 {
        let a = p[3];
        p.copy_from_slice(&[255, 255, 255, a]);
    }
    Some(ImageData::from_rgba8(px, px, out))
}
