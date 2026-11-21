//! Barra de cima: voltar + título · episódio, com fade quando os controles somem.

use higanbana_core::player::logic::episode_label;
use higanbana_gpui::components::{Button, Component, Variant};
use higanbana_gpui::prelude::*;

use super::style::scrim_down;
use crate::chrome::tip;
use crate::root::Higanbana;
use crate::theme::on_media_muted;
use crate::ui::truncate;

pub fn top_bar(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, fade: f32) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let title = p.session.target.anime_title.clone();
    let ep = episode_label(&p.session.target.file.name);
    let sm = cx.theme().typography.sm;
    let back = Button::new("player-voltar", "").variant(Variant::Ghost).icon(icon("arrow-left").size(16.0)).on_click(|v: &mut Higanbana, _, cx| v.close_player(cx)).render(cx);
    let back = tip(v, cx, "dica:player-voltar", back, "Voltar (Esc)");
    div()
        .flex_shrink_0()
        .items_center()
        .gap(8.0)
        .h(48.0)
        .pl(8.0)
        .pr(16.0)
        .bg_gradient(scrim_down(0.75))
        .opacity(fade)
        .child(back)
        .child(
            div()
                .min_w(0.0)
                .flex_1()
                .items_center()
                .text_size(sm)
                .child(truncate(title).flex_shrink_0().max_w(pct(70.0)).font_weight(600))
                .child(div().flex_shrink_0().text_color(on_media_muted()).child(text(format!(" · {ep}")))),
        )
}
