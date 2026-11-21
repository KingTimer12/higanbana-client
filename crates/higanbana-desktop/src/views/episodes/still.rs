//! Miniatura do card: still do TMDB (pedido só quando aparece), esqueleto
//! enquanto o TMDB responde, fundo neutro sem dados; e o fundo borrado da lista.

use higanbana_gpui::components::{Component, Skeleton};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};
use higanbana_gpui::{Corners, ObjectFit};

use super::Item;
use crate::motion::{transition, QUICK};
use crate::root::Higanbana;
use crate::ui::image::{look, Look, PREFETCH};

/// Superfície do tema com o ícone de vídeo.
fn neutral(t: &Theme, radii: Corners<f32>) -> Element<Higanbana> {
    let g = LinearGradient::new(GradientDir::Right, vec![(0.0, t.muted), (1.0, t.secondary)]);
    div().size_full().rounded_corners(radii).items_center().justify_center().bg_gradient(g).child(icon("file-play").size(16.0).text_color(t.muted_foreground))
}

/// Imagem cobrindo a caixa, ampliada `zoom` a partir do centro, com os cantos `radii`.
pub fn still(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, it: &Item, loading: bool, radii: Corners<f32>, zoom: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    if loading {
        return Skeleton::new(("ep-esqueleto", it.path.as_str())).w(pct(100.0)).h(pct(100.0)).render(cx).rounded_corners(radii);
    }
    let Some(url) = &it.still else { return neutral(&t, radii) };
    match look(v.state.cached_image(url)) {
        Look::Show(src) => {
            let a = cx.animate_from(("ep-img", url.as_str()), 0.0, 1.0, transition(QUICK));
            img(src).size_full().object_fit(ObjectFit::Cover).zoom(zoom).rounded_corners(radii).opacity(a)
        }
        Look::Neutral => neutral(&t, radii),
        Look::Wait => div().size_full(),
        Look::Request => {
            let url = url.clone();
            div().size_full().relative().child(on_visible(PREFETCH, move |v: &mut Higanbana, cx| {
                v.state.image(&url);
                cx.notify();
            }))
        }
    }
}

/// Fundo borrado do card da lista: o still minúsculo esticado, fraco (a ampliação
/// faz o papel do desfoque). Só é pedido no primeiro hover.
pub fn bleed(v: &mut Higanbana, it: &Item, radius: f32, e: f32) -> Option<Element<Higanbana>> {
    let url = it.bleed.as_ref()?;
    let src = match v.state.cached_image(url) {
        Some(crate::state::Img::Ready(src)) => src,
        Some(_) => return None,
        None => {
            v.state.image(url);
            return None;
        }
    };
    Some(div().absolute().top(0.0).left(0.0).size_full().opacity(0.35 * e).child(img(src).size_full().object_fit(ObjectFit::Cover).zoom(1.4).rounded(radius)))
}
