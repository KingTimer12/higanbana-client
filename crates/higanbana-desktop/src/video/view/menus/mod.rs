//! Os três menus da barra (popover ancorado embaixo à direita, botão ghost com dica).

mod episodes;
pub(super) mod settings;
pub(super) mod tracks;

use higanbana_gpui::components::{Button, Component, Popover, PopoverAnchor, Switch, Variant};
use higanbana_gpui::prelude::*;

use crate::chrome::tip;
use crate::root::Higanbana;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Menu {
    Tracks,
    Settings,
    Episodes,
}

const MENUS: [(Menu, &str, &str, &str, f32); 3] = [
    (Menu::Tracks, "player-faixas", "captions", "Legenda e áudio", 288.0),
    (Menu::Settings, "player-ajustes", "settings-2", "Ajustes", 320.0),
    (Menu::Episodes, "player-episodios", "list-video", "Episódios", 384.0),
];

/// Linha "chave + rótulo" (o Switch da lib não tem rótulo; clicar no texto também alterna).
pub(super) fn switch_row(id: &'static str, label: &'static str, on: bool, cx: &mut RenderCx<'_, Higanbana>, f: fn(&mut Higanbana, bool)) -> Element<Higanbana> {
    let sw = Switch::new(id, on)
        .on_change(move |v: &mut Higanbana, b, cx| {
            f(v, b);
            cx.notify();
        })
        .render(cx);
    let sm = cx.theme().typography.sm;
    div()
        .items_center()
        .gap(8.0)
        .cursor_pointer()
        .child(sw)
        .child(div().text_size(sm).child(text(label)).on_click(move |v: &mut Higanbana, _, cx| {
            f(v, !on);
            cx.notify();
        }))
}

pub fn menus(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Vec<Element<Higanbana>> {
    let Some(p) = v.player.as_mut() else { return Vec::new() };
    // O menu aberto é o do popover aberto (fecha os outros).
    let open = MENUS.iter().enumerate().find(|(i, _)| p.pops[*i].open).map(|(_, m)| m.0);
    p.menu = open;
    let mut out = Vec::new();
    for (i, (menu, id, name, hint, w)) in MENUS.into_iter().enumerate() {
        let content = match menu {
            Menu::Tracks => tracks::content(v, cx),
            Menu::Settings => settings::content(v, cx),
            Menu::Episodes => episodes::content(v, cx),
        };
        let trigger = Button::new(format!("{id}-botao"), "").variant(Variant::Ghost).icon(icon(name).size(16.0)).render(cx);
        let trigger = tip(v, cx, format!("dica:{id}").as_str(), trigger, hint);
        let Some(p) = v.player.as_ref() else { break };
        let state = p.pops[i];
        let pop = Popover::new(id, &state, move |v: &mut Higanbana| &mut v.player.as_mut().expect("player aberto").pops[i], trigger)
            .anchor(PopoverAnchor::BottomRight)
            .w(w)
            .content(move |_| content)
            .render(cx);
        out.push(pop);
    }
    out
}
