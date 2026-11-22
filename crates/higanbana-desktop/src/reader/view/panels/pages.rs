//! Ir para a página: grade de 6 colunas com a atual marcada.

use higanbana_gpui::components::{Button, Component, ComponentSize, Variant};
use higanbana_gpui::prelude::*;

use super::{card, heading};
use crate::root::Higanbana;

const COLS: usize = 6;
const W: f32 = 320.0;
const GAP: f32 = 4.0;

pub fn panel(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let accent = cx.theme().accent;
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let (total, current) = (r.total(), r.page());
    // Sem grade na lib: largura de cada célula calculada.
    let cell = ((W - 32.0 - GAP * (COLS - 1) as f32) / COLS as f32).floor();
    let mut grid = div().flex_wrap().gap(GAP);
    for pg in 0..total {
        let b = Button::new(("ir-pagina", pg), (pg + 1).to_string()).variant(Variant::Ghost).size(ComponentSize::Sm).on_click(move |v: &mut Higanbana, _, cx| {
            if let Some(r) = v.state.reader.as_mut() {
                r.go_page(pg);
            }
            v.state.pump_reader();
            cx.notify();
        });
        let b = b.render(cx).w(cell).px(0.0);
        grid = grid.child(if pg == current { b.bg(accent) } else { b });
    }
    let body = scroll("leitor-paginas").p(16.0).child(heading(cx, "Ir para a página")).child(grid);
    card(cx, "leitor-painel-paginas").top(16.0).right(16.0).w(W).max_h(480.0).child(body)
}
