//! Popover com menu de faixas (como no player).
use super::preview;
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

pub fn popover(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let trigger = div()
        .h(36.0)
        .px(12.0)
        .gap(8.0)
        .items_center()
        .rounded(theme.radius - 2.0)
        .border(1.0, theme.input)
        .text_size(14.0)
        .child(icon("captions"))
        .child(text("Legendas"));
    let sel = v.subtitle;
    let pop = Popover::new("pop-faixas", &v.popover, |v: &mut Gallery| &mut v.popover, trigger)
        .anchor(PopoverAnchor::TopLeft)
        .w(288.0)
        .content(move |cx| {
            let radios = RadioGroup::new("pop-rg", sel)
                .items(["Desligada", "Português (Brasil)", "Inglês"])
                .on_change(|v: &mut Gallery, i, cx| {
                    v.subtitle = Some(i);
                    cx.notify();
                })
                .render(cx);
            div().flex_col().gap(10.0).child(text("Legendas").text_size(12.0).font_weight(500).text_color(muted)).child(radios)
        })
        .render(cx);
    vec![preview(cx, "Menu", pop)]
}
