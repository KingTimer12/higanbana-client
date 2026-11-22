//! Formas preenchidas (polígonos) rasterizadas na CPU como máscara branca + alfa,
//! tingidas no desenho; e o recorte por uv de imagens com `cover`/zoom.
use crate::element::{ImageData, ImageSource};
use crate::geometry::Rect;
use crate::paint::cache::{self, AgedMap};
use std::cell::RefCell;

/// Pontos quantizados em 1/1024 da caixa: formas animadas reusam o cache.
const Q: f32 = 1024.0;

type Key = (u32, u32, Vec<(i32, i32)>);

thread_local! {
    /// Some quando a forma deixa de ser pintada por alguns frames (`AgedMap::sweep`).
    static CACHE: RefCell<AgedMap<Key>> = RefCell::new(AgedMap::default());
}

/// Máscara do polígono `points` (frações 0–1 da caixa) com `w`×`h` px físicos.
pub fn get(points: &[(f32, f32)], w: u32, h: u32) -> Option<ImageSource> {
    let key: Key = (w, h, points.iter().map(|(x, y)| ((x * Q).round() as i32, (y * Q).round() as i32)).collect());
    if let Some(hit) = CACHE.with(|c| c.borrow_mut().get(&key)) {
        return hit;
    }
    cache::register(sweep);
    let made = rasterize(points, w, h);
    CACHE.with(|c| c.borrow_mut().insert(key, made.clone()));
    made
}

fn sweep(now: u64) {
    CACHE.with(|c| c.borrow_mut().sweep(now));
}

/// Rasteriza sem cache; `None` com menos de 3 pontos ou caixa vazia.
pub(crate) fn rasterize(points: &[(f32, f32)], w: u32, h: u32) -> Option<ImageSource> {
    use resvg::tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};
    if points.len() < 3 || w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let (fw, fh) = (w as f32, h as f32);
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0 * fw, points[0].1 * fh);
    for (x, y) in &points[1..] {
        pb.line_to(x * fw, y * fh);
    }
    pb.close();
    let path = pb.finish()?;
    let mut pix = Pixmap::new(w, h)?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;
    pix.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    // Só a cobertura importa: branco + alfa, tingido no desenho.
    let mut out = pix.take();
    for p in out.as_chunks_mut::<4>().0 {
        let a = p[3];
        p.copy_from_slice(&[255, 255, 255, a]);
    }
    Some(ImageData::from_rgba8(w, h, out))
}

/// Parte da imagem (`img` px) que cobre a caixa `boxed` com `cover`, ampliada
/// `zoom` vezes a partir do centro, em coordenadas uv (0–1).
pub fn cover_uv(boxed: (f32, f32), img: (u32, u32), zoom: f32) -> Rect {
    let (iw, ih) = (img.0.max(1) as f32, img.1.max(1) as f32);
    let (bw, bh) = (boxed.0.max(1e-3), boxed.1.max(1e-3));
    let s = (bw / iw).max(bh / ih) * zoom.max(1e-3);
    let (uw, uh) = ((bw / s / iw).min(1.0), (bh / s / ih).min(1.0));
    Rect::new((1.0 - uw) / 2.0, (1.0 - uh) / 2.0, uw, uh)
}
