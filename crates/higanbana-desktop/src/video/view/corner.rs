//! Canto inferior direito: "Pular abertura" e o cartão do próximo episódio
//! (ficam mesmo com os controles escondidos, só descem).

use higanbana_core::player::logic::episode_number;
use higanbana_gpui::components::{Button, Component, Progress, ProgressSize, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use super::style::media_card;
use crate::chrome::tip;
use crate::motion;
use crate::root::Higanbana;
use crate::ui::truncate;

/// Texto do cabeçalho do cartão.
pub fn next_heading(countdown: Option<f64>) -> String {
    match countdown {
        Some(left) => format!("Próximo episódio em {} s", left.ceil() as u32),
        None => "Próximo episódio".into(),
    }
}

/// Barra da contagem (0..100).
pub fn countdown_fill(left: f64, total: f64) -> f32 {
    ((1.0 - left / total.max(1.0)) * 100.0) as f32
}

fn next_card(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let label = p.session.next().map_or_else(String::new, |n| episode_number(&n.name).map_or_else(|| n.name.clone(), |num| format!("Episódio {num}")));
    let cd = p.session.countdown();
    let t = cx.theme().clone();
    let alpha = cx.animate_from(ElementId::from("player-proximo-cartao"), 0.0, 1.0, motion::transition(motion::BASE));
    let watch = Button::new("player-assistir", "Assistir agora")
        .variant(Variant::Default)
        .icon(icon("play").size(16.0))
        .on_click(|v: &mut Higanbana, _, cx| {
            if let Some(p) = v.player.as_mut() {
                let a = p.session.go_next(std::time::Instant::now());
                p.run(a);
            }
            cx.notify();
        })
        .render(cx);
    let cancel = Button::new("player-cancelar", "Cancelar")
        .variant(Variant::Ghost)
        .on_click(|v: &mut Higanbana, _, cx| {
            if let Some(p) = v.player.as_mut() {
                p.session.cancel_next();
            }
            cx.notify();
        })
        .render(cx);
    let mut card = media_card(&t)
        .id("player-proximo")
        .w(320.0)
        .opacity(alpha)
        .on_click(|_: &mut Higanbana, _, cx| cx.stop_propagation())
        .child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(next_heading(cd.map(|c| c.0)))))
        .child(truncate(label).font_weight(600));
    if let Some((left, total)) = cd {
        card = card.child(Progress::new("player-contagem-barra").size(ProgressSize::XSmall).value(countdown_fill(left, total)).render(cx));
    }
    card.child(div().gap(8.0).child(div().flex_1().child(watch)).child(cancel))
}

pub fn corner(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, hidden: bool) -> Element<Higanbana> {
    let Some(p) = v.player.as_ref() else { return div() };
    let show_next = p.session.show_next();
    let show_skip = p.session.show_skip() && !show_next;
    let mut col = div().absolute().right(20.0).bottom(if hidden { 24.0 } else { 96.0 }).flex_col().items_end().gap(12.0);
    if show_skip {
        let b = Button::new("player-pular", "Pular abertura")
            .variant(Variant::Secondary)
            .icon(icon("chevrons-right").size(16.0))
            .on_click(|v: &mut Higanbana, _, cx| {
                cx.stop_propagation();
                if let Some(p) = v.player.as_ref()
                    && let Some(to) = p.session.skip_intro()
                {
                    p.player.seek(to);
                }
                cx.notify();
            })
            .render(cx);
        col = col.child(tip(v, cx, "dica:player-pular", b, "Pular abertura (S)"));
    }
    if show_next {
        col = col.child(next_card(v, cx));
    }
    col
}
