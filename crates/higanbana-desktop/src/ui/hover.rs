//! Hover com fade (como o `HoverFade` da master): o elemento rastreia o mouse
//! (`on_hover` que só reconstrói) e uma camada por cima/baixo acende em QUICK.

use higanbana_gpui::prelude::*;
use higanbana_gpui::{Corners, ElementId};

use crate::motion::{transition, QUICK};
use crate::root::Higanbana;

/// Marca `e` para saber quando o mouse entra/sai (reconstrói só nessas horas).
pub fn track(e: Element<Higanbana>, id: impl Into<ElementId>) -> Element<Higanbana> {
    e.id(id).on_hover(|_: &mut Higanbana, _, cx| cx.notify())
}

/// Progresso 0–1 do hover de `id` (marcado com `track`).
pub fn amount(cx: &mut RenderCx<'_, Higanbana>, id: impl Into<ElementId>) -> f32 {
    let id = id.into();
    let on = cx.is_hovered(id);
    cx.animate(ElementId(id.0.rotate_left(13) ^ 0x686f_7665_72), if on { 1.0 } else { 0.0 }, transition(QUICK))
}

/// Camada que cobre o pai (`relative`): fundo `fill` e/ou borda de 1 px `border`, na opacidade `t`.
pub fn layer(t: f32, radius: Corners<f32>, fill: Option<Color>, border: Option<Color>) -> Element<Higanbana> {
    let mut d = div().absolute().top(0.0).left(0.0).size_full().rounded_corners(radius).opacity(t);
    if let Some(c) = fill {
        d = d.bg(c);
    }
    if let Some(c) = border {
        d = d.border(1.0, c);
    }
    d
}
