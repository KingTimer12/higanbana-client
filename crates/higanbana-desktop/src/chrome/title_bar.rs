//! Barra de título: marca, versão, voltar/avançar, busca, atualização e downloads.

use higanbana_gpui::components::{Button, Component, ComponentSize, Kbd, TitleBar, Variant};
use higanbana_gpui::prelude::*;

use super::{downloads, tip, update};
use crate::root::Higanbana;

/// "higanbana" em negrito na cor da marca (também na tela de login).
pub fn brand(cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    text("higanbana").font_weight(700).text_color(cx.theme().primary)
}

fn nav_button(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: &'static str, name: &'static str, label: &'static str, enabled: bool, back: bool) -> Element<Higanbana> {
    let b = Button::new(id, "")
        .variant(Variant::Ghost)
        .size(ComponentSize::Compact)
        .icon(icon(name).size(14.0))
        .disabled(!enabled)
        .on_click(move |v: &mut Higanbana, _, cx| {
            if back { v.state.back() } else { v.state.forward() }
            cx.notify();
        })
        .render(cx);
    tip(v, cx, ("dica", id), b, label)
}

/// Botão que parece um campo e abre a paleta.
fn search_trigger(cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let muted = cx.theme().muted_foreground;
    let kbd = Kbd::new("ctrl-k").render(cx);
    let inner = div()
        .w_full()
        .items_center()
        .gap(8.0)
        .text_color(muted)
        .child(icon("search").size(14.0))
        .child(div().flex_1().child(text("Buscar animes, mangás, OSTs…")))
        .child(kbd);
    let b = Button::new("search", "").variant(Variant::Outline).size(ComponentSize::Compact).on_click(|v: &mut Higanbana, _, cx| {
        v.open_search(cx);
    });
    // Largura do kit (`w_96`) e conteúdo próprio dentro do botão.
    div().w(384.0).child(b.render(cx).w_full().justify_start().child(inner))
}

pub fn title_bar(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let (can_back, can_fwd) = (v.state.nav.can_back(), v.state.nav.can_forward());
    let muted = cx.theme().muted_foreground;
    let xs = cx.theme().typography.xs;
    let row = div()
        .w_full()
        .h_full()
        .items_center()
        .pr(8.0)
        .gap(4.0)
        .child(brand(cx))
        .child(div().pr(12.0).text_size(xs).text_color(muted).child(text(format!("v{}", higanbana_core::version::current()))))
        .child(nav_button(v, cx, "back", "chevron-left", "Voltar (Alt+←)", can_back, true))
        .child(nav_button(v, cx, "forward", "chevron-right", "Avançar (Alt+→)", can_fwd, false))
        .child(div().flex_1().justify_center().child(search_trigger(cx)))
        .child(update::button(v, cx))
        .child(downloads::menu(v, cx));
    TitleBar::new("").kit().content(row).render(cx)
}
