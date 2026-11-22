//! RadioGroup.
use super::preview;
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

pub fn radio(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let subs = RadioGroup::new("rg-legenda", v.subtitle)
        .items(["Desligada", "Português (Brasil)", "Inglês"])
        .on_change(|v: &mut Gallery, i, cx| {
            v.subtitle = Some(i);
            cx.notify();
        })
        .render(cx);
    let q = RadioGroup::new("rg-cache", v.quality)
        .items(["64 MB", "128 MB", "256 MB", "512 MB"])
        .horizontal()
        .on_change(|v: &mut Gallery, i, cx| {
            v.quality = Some(i);
            cx.notify();
        })
        .render(cx);
    vec![preview(cx, "Vertical", subs), preview(cx, "Horizontal", q)]
}
