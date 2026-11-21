//! Progress e Spinner.
use super::{preview, row};
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

pub fn progress(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let mut bars = div().flex_col().gap(16.0).w(360.0);
    for (i, s) in [ProgressSize::XSmall, ProgressSize::Small, ProgressSize::Medium, ProgressSize::Large].into_iter().enumerate() {
        bars = bars.child(Progress::new(("pg", i)).value(v.progress).size(s).render(cx));
    }
    let less = Button::new("pg-menos", "−10").variant(Variant::Outline).on_click(|v: &mut Gallery, _, cx| {
        v.progress = (v.progress - 10.0).max(0.0);
        cx.notify();
    });
    let more = Button::new("pg-mais", "+10").variant(Variant::Outline).on_click(|v: &mut Gallery, _, cx| {
        v.progress = (v.progress + 10.0).min(100.0);
        cx.notify();
    });
    let buttons = row().child(less.render(cx)).child(more.render(cx));
    let spinners = row()
        .gap(24.0)
        .child(Spinner::new("sp-12").size(12.0).render(cx))
        .child(Spinner::new("sp-16").render(cx))
        .child(Spinner::new("sp-24").size(24.0).render(cx))
        .child(Spinner::new("sp-p").size(24.0).color(cx.theme().primary).render(cx));
    vec![preview(cx, "Tamanhos", div().flex_col().gap(20.0).items_center().child(bars).child(buttons)), preview(cx, "Spinner", spinners)]
}
