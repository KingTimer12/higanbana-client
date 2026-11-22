//! Formas dos cards: cantos de pétala, bolha do ▶, faixa de progresso que segue
//! os cantos de baixo e o corte diagonal da miniatura da lista. Em `f32`; os
//! polígonos saem em frações (0–1) da caixa, como o `polygon` da lib pede.

use higanbana_gpui::Corners;

/// Cantos da miniatura da grade, estreito e largo alternados; no hover trocam.
pub const PETAL_TIGHT: f32 = 4.;
pub const PETAL_WIDE: f32 = 22.;
/// Pétala do ▶: diâmetro aberto e distância do centro dela ao canto.
pub const PETAL_SIZE: f32 = 90.;
pub const PETAL_INSET: f32 = 15.;
/// Corte diagonal da miniatura da lista: a borda direita termina embaixo a 86 %.
pub const LIST_CUT: f32 = 0.14;
/// Altura da barra de progresso.
pub const BAR_HEIGHT: f32 = 3.;

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Raios da miniatura da grade com o hover em `t` (0–1).
pub fn petal_corners(t: f32) -> Corners<f32> {
    let (a, b) = (lerp(PETAL_TIGHT, PETAL_WIDE, t), lerp(PETAL_WIDE, PETAL_TIGHT, t));
    Corners { top_left: a, top_right: b, bottom_right: a, bottom_left: b }
}

/// A parte visível da bolha de raio `radius` centrada `PETAL_INSET` para dentro do canto.
#[derive(Debug, PartialEq)]
pub struct Bubble {
    pub offset: f32,
    pub side: f32,
    pub arc: f32,
    pub rest: f32,
}

pub fn bubble(radius: f32) -> Bubble {
    let radius = radius.max(0.);
    let offset = (PETAL_INSET - radius).max(0.);
    Bubble { offset, side: PETAL_INSET + radius - offset, arc: radius, rest: if offset > 0. { radius } else { 0. } }
}

/// Faixa de baixo de um retângulo `w`×`h` de cantos de baixo `bl`/`br`, com `bar`
/// de altura e cheia até `fraction`: pontos em px seguindo a curva dos cantos.
pub fn bottom_strip(w: f32, h: f32, bl: f32, br: f32, bar: f32, fraction: f32) -> Vec<(f32, f32)> {
    let end = w * fraction.clamp(0., 1.);
    if end <= 0. || w <= 0. || h <= 0. {
        return Vec::new();
    }
    let limit = (w / 2.).min(h / 2.);
    let (bl, br) = (bl.clamp(0., limit), br.clamp(0., limit));
    let inset = |r: f32, y: f32| {
        let dy = y - (h - r);
        if r <= 0. || dy <= 0. { 0. } else { r - (r * r - dy * dy).max(0.).sqrt() }
    };
    const STEPS: usize = 6;
    let ys: Vec<f32> = (0..=STEPS).map(|i| h - bar + bar * i as f32 / STEPS as f32).collect();
    let left = ys.iter().map(|&y| (inset(bl, y), y));
    let right = ys.iter().rev().map(|&y| ((w - inset(br, y)).min(end).max(inset(bl, y)), y));
    left.chain(right).collect()
}

/// `bottom_strip` em frações da caixa.
pub fn strip_fractions(w: f32, h: f32, bl: f32, br: f32, fraction: f32) -> Vec<(f32, f32)> {
    bottom_strip(w, h, bl, br, BAR_HEIGHT, fraction).into_iter().map(|(x, y)| (x / w, y / h)).collect()
}

/// Triângulo do corte diagonal (frações): de cima à direita até `1 - corte` embaixo.
pub fn cut_triangle(amount: f32) -> Option<[(f32, f32); 3]> {
    let cut = LIST_CUT * amount.clamp(0., 1.);
    (cut > 0.).then_some([(1.0, 0.0), (1.0, 1.0), (1.0 - cut, 1.0)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn petal_corners_swap_on_hover() {
        let (rest, hover) = (petal_corners(0.), petal_corners(1.));
        assert_eq!((rest.top_left, rest.top_right, rest.bottom_right, rest.bottom_left), (4., 22., 4., 22.));
        assert_eq!((hover.top_left, hover.top_right, hover.bottom_right, hover.bottom_left), (22., 4., 22., 4.));
        assert_eq!(petal_corners(0.5).top_left, 13.);
    }

    #[test]
    fn open_bubble_is_cut_by_the_corner() {
        assert_eq!(bubble(45.), Bubble { offset: 0., side: 60., arc: 45., rest: 0. });
    }

    #[test]
    fn small_bubble_is_a_whole_circle() {
        assert_eq!(bubble(9.), Bubble { offset: 6., side: 18., arc: 9., rest: 9. });
        assert_eq!(bubble(-1.), Bubble { offset: 15., side: 0., arc: 0., rest: 0. });
    }

    #[test]
    fn strip_without_radius_is_the_bottom_rectangle() {
        let pts = bottom_strip(100., 50., 0., 0., 3., 0.4);
        assert!(pts.iter().all(|p| p.0 == 0. || p.0 == 40.));
        assert!(pts.iter().all(|p| (47. ..=50.).contains(&p.1)));
    }

    #[test]
    fn strip_follows_the_rounded_corners() {
        let pts = bottom_strip(200., 100., 22., 4., 3., 1.);
        let bl = pts.iter().find(|p| p.1 == 100.).unwrap();
        assert!((bl.0 - 22.).abs() < 1e-3);
        let br = pts.iter().rev().find(|p| p.1 == 100.).unwrap();
        assert!((br.0 - 196.).abs() < 1e-3);
        assert!(bottom_strip(200., 100., 22., 4., 3., 0.).is_empty());
    }

    #[test]
    fn diagonal_cut_opens_on_hover() {
        let [top, bottom, inner] = cut_triangle(1.).unwrap();
        assert_eq!((top, bottom), ((1.0, 0.0), (1.0, 1.0)));
        assert!((inner.0 - 0.86).abs() < 1e-6);
        assert!(cut_triangle(0.).is_none());
    }
}
