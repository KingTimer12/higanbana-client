//! Aviso de versão nova na barra de título (some quando não há atualização).

use higanbana_gpui::components::{Button, Component, ComponentSize, Variant};
use higanbana_gpui::prelude::*;

use super::tip;
use crate::root::Higanbana;
use crate::state::UpdateState;

pub fn button(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let (label, hint, busy) = match &v.state.update {
        UpdateState::Idle => return div(),
        UpdateState::Available(r) => (format!("Atualizar para v{}", r.version), "Baixar e instalar a nova versão (o app reinicia)", false),
        UpdateState::Downloading(r) => (format!("Baixando v{}...", r.version), "Baixando e conferindo a atualização", true),
    };
    let b = Button::new("update", label)
        .variant(Variant::Default)
        .size(ComponentSize::Compact)
        .icon(icon("download").size(14.0))
        .disabled(busy)
        .on_click(|v: &mut Higanbana, _, cx| {
            v.state.install_update();
            cx.notify();
        })
        .render(cx);
    tip(v, cx, "dica-update", b, hint)
}
