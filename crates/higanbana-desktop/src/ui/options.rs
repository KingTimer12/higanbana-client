//! Grupos de opções (temporada, versão) e o cartão de versão da obra.

use higanbana_core::library::VersionLabel;
use higanbana_gpui::components::{Button, Component, ComponentSize, Tag, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{Corners, ElementId};

use super::{hover, truncate};
use crate::root::Higanbana;

/// Grupo com rótulo à esquerda.
pub fn option_group(t: &Theme, label: &'static str) -> Element<Higanbana> {
    div().items_start().gap(16.0).child(div().w(80.0).flex_shrink_0().pt(4.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(text(label)))
}

/// Uma opção de um grupo: botão outline pequeno, destacado quando escolhido.
pub fn option_button(cx: &mut RenderCx<'_, Higanbana>, id: impl Into<ElementId>, label: impl Into<std::borrow::Cow<'static, str>>, selected: bool, f: impl Fn(&mut Higanbana, &mut EventCx) + 'static) -> Element<Higanbana> {
    let accent = cx.theme().accent;
    let b = Button::new(id, label).variant(Variant::Outline).size(ComponentSize::Compact).on_click(move |v: &mut Higanbana, _, cx| {
        f(v, cx);
        cx.notify();
    });
    let b = b.render(cx);
    if selected { b.bg(accent) } else { b }
}

/// Cartão de uma versão: nome e etiquetas de resolução, fonte, áudio e episódios.
pub fn version_card(cx: &mut RenderCx<'_, Higanbana>, id: impl Into<ElementId>, label: &VersionLabel, episodes: Option<usize>, selected: bool) -> Element<Higanbana> {
    let id = id.into();
    let t = cx.theme().clone();
    let tags = [label.resolution.clone(), label.source.clone(), label.audio.clone(), episodes.map(|n| format!("{n} ep"))];
    let mut row = div().flex_wrap().gap(6.0);
    for tag in tags.into_iter().flatten() {
        row = row.child(Tag::secondary(tag).xsmall().render(cx));
    }
    let mut card = div()
        .relative()
        .flex_col()
        .min_w(160.0)
        .max_w(320.0)
        .gap(4.0)
        .px(12.0)
        .py(8.0)
        .rounded(t.radius_lg)
        .cursor_pointer()
        .child(truncate(label.name.clone()).text_size(t.typography.sm).font_weight(600))
        .child(row);
    card = if selected { card.border(1.0, t.primary).bg(t.primary.fade(0.1)) } else { card.border(1.0, t.border).bg(t.muted) };
    if !selected {
        card = card.child(hover::layer(hover::amount(cx, id), Corners::all(t.radius_lg), None, Some(t.ring)));
    }
    hover::track(card, id)
}
