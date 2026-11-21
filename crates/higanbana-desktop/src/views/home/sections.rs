//! Seções da raiz da nuvem em cartões baixos: selo na cor da seção e nome;
//! no hover o cartão sobe 3 px e a borda pega a cor.

use higanbana_gpui::components::{Component, Skeleton};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use crate::chrome::nav_entries;
use crate::motion::{ease_out_cubic, rise, RISE_DISTANCE};
use crate::root::Higanbana;
use crate::ui::cover::columns;
use crate::ui::hover;
use crate::ui::SHADOW_MD;

/// Cor de cada seção (tokens do tema): dá identidade aos atalhos.
fn hue(t: &Theme, name: &str) -> Color {
    match name {
        "Animes" => t.primary,
        "Mangás" => t.base.blue,
        "Doramas" => t.base.magenta,
        "Desenhos" => t.base.yellow,
        "OSTs" => t.base.green,
        _ => t.base.cyan,
    }
}

const GAP: f32 = 12.0;

pub fn grid(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, width: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let list = v.state.sections.clone();
    let count = if list.is_empty() { 6 } else { list.len() };
    let cols = columns(width, 144.0, GAP).min(count).max(1);
    let card_w = (width - GAP * (cols as f32 - 1.0)) / cols as f32;
    let mut grid = div().flex_wrap().gap(GAP);
    if list.is_empty() {
        for i in 0..6 {
            grid = grid.child(Skeleton::new(("secao-esqueleto", i)).w(card_w).h(64.0).render(cx).rounded(t.radius_lg));
        }
        return grid;
    }
    for (i, e) in nav_entries(&list).into_iter().enumerate() {
        let id = ElementId::from(("secao", e.target.as_str()));
        let h = ease_out_cubic(hover::amount(cx, id));
        let color = hue(&t, &e.section);
        let enter = rise(cx, ("secao-entra", e.target.as_str()), i);
        let badge = div().w(36.0).h(36.0).flex_shrink_0().items_center().justify_center().rounded(t.radius).bg(color.fade(0.14 + 0.12 * h)).child(icon(e.icon).size(14.0).text_color(color));
        let chevron = div().absolute().right(8.0).top(0.0).bottom(0.0).items_center().child(icon("chevron-right").size(14.0).text_color(t.muted_foreground.fade(h)));
        let ring = div().absolute().top(0.0).left(0.0).size_full().rounded(t.radius_lg).border(1.0, color.fade(0.6 * h));
        let mut card = div()
            .relative()
            .w(card_w)
            .h(64.0)
            .px(12.0)
            .gap(12.0)
            .items_center()
            .rounded(t.radius_lg)
            .border(1.0, t.border)
            .bg(t.group_box)
            .cursor_pointer()
            .opacity(enter)
            .translate_y(RISE_DISTANCE * (1.0 - enter) - 3.0 * h)
            .child(badge)
            .child(div().flex_1().min_w(0.0).line_clamp(1).flex_col().font_weight(500).child(text(e.label.clone())))
            .child(chevron)
            .child(ring);
        if h > 0.01 {
            card = card.shadow(Color::BLACK.fade(0.1), SHADOW_MD);
        }
        let target = e.target.clone();
        let card = card.on_click(move |v: &mut Higanbana, _, cx| {
            v.state.go(target.clone());
            cx.notify();
        });
        grid = grid.child(hover::track(card, id));
    }
    grid
}
