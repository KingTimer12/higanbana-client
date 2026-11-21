//! Rampa do gradiente rasterizada na CPU (256 amostras) e cacheada; a GPU estica com filtro linear.
use crate::element::{ImageData, ImageSource};
use crate::style::{GradientDir, LinearGradient};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

const N: u32 = 256;

thread_local! {
    static CACHE: RefCell<HashMap<u64, ImageSource>> = RefCell::new(HashMap::new());
}

fn key(g: &LinearGradient) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.dir.hash(&mut h);
    for (p, c) in &g.stops {
        for f in [*p, c.r, c.g, c.b, c.a] {
            f.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// Imagem da rampa (1×256 vertical, 256×1 horizontal), alfa reto.
pub fn ramp(g: &LinearGradient) -> ImageSource {
    let k = key(g);
    if let Some(src) = CACHE.with(|c| c.borrow().get(&k).cloned()) {
        return src;
    }
    let mut px = Vec::with_capacity(N as usize * 4);
    for i in 0..N {
        let t = i as f32 / (N - 1) as f32;
        let t = if matches!(g.dir, GradientDir::Up | GradientDir::Left) { 1.0 - t } else { t };
        let c = g.sample(t);
        px.extend([c.r, c.g, c.b, c.a].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
    }
    let src = match g.dir {
        GradientDir::Down | GradientDir::Up => ImageData::from_rgba8(1, N, px),
        GradientDir::Right | GradientDir::Left => ImageData::from_rgba8(N, 1, px),
    };
    CACHE.with(|c| c.borrow_mut().insert(k, src.clone()));
    src
}
