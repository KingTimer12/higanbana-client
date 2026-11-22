//! Índice lateral de letras: pula até a letra; na grade única fica desabilitado.

use higanbana_core::anime_index::LETTERS;
use higanbana_gpui::components::{item_offset, Button, Component, ComponentSize, Variant};
use higanbana_gpui::prelude::*;

use super::rows::{letter_row, rail_label, Row};
use crate::root::Higanbana;

pub fn rail(cx: &mut RenderCx<'_, Higanbana>, rows: &[Row], heights: &std::sync::Arc<[f32]>, flat: bool, searching: bool, present: &[usize], current: Option<usize>) -> Element<Higanbana> {
    let accent = cx.theme().accent;
    let mut col = div().flex_col().items_center().py(16.0);
    for (ix, l) in LETTERS.iter().enumerate() {
        // Já na lista: rola até o título; sem busca e ainda não desenhada, pede e rola depois.
        let offset = letter_row(rows, ix).map(|i| item_offset(heights, 0.0, i));
        let b = Button::new(("indice", ix), rail_label(l).to_owned())
            .variant(Variant::Ghost)
            .size(ComponentSize::Xs)
            .disabled(flat || (searching && !present.contains(&ix)))
            .on_click(move |v: &mut Higanbana, _, cx| super::jump(v, ix, offset.unwrap_or(f32::MAX), cx))
            .render(cx);
        col = col.child(if current == Some(ix) { b.bg(accent) } else { b });
    }
    // A margem automática centraliza; se não couber, o índice rola sem cortar o topo.
    scroll("catalogo-indice").flex_shrink_0().w(40.0).h_full().items_center().child(div().flex_col().flex_1().justify_center().child(col))
}
