//! Ícones SVG (Lucide) e gradiente de fundo.
use super::{preview, row};
use crate::Gallery;
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};

const NAMES: &[&str] = &[
    "play", "pause", "skip-forward", "volume-2", "volume-x", "captions", "settings-2", "list-video", "maximize", "search",
    "house", "folder", "star", "download", "refresh-cw", "circle-alert", "wifi-off", "music", "book-open", "log-out",
];

pub fn icons(_: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Vec<Element<Gallery>> {
    let theme = cx.theme();
    let grid = row().children(NAMES.iter().map(|n| icon(n).size(20.0)));
    let tinted = row()
        .child(icon("star").size(28.0).text_color(theme.base.yellow))
        .child(icon("circle-alert").size(28.0).text_color(theme.destructive_foreground))
        .child(icon("folder-check").size(28.0).text_color(theme.success_foreground))
        .child(icon("play").size(28.0).text_color(theme.primary));
    let scrim = div()
        .w(420.0)
        .h(160.0)
        .rounded(theme.radius_lg)
        .overflow_hidden()
        .bg(theme.primary)
        .flex_col()
        .justify_between()
        .child(div().h(48.0).px(16.0).items_center().bg_gradient(LinearGradient::scrim(GradientDir::Down, Color::BLACK.alpha(0.75), Color::TRANSPARENT)).child(text("Topo").text_color(Color::WHITE)))
        .child(div().h(64.0).px(16.0).items_end().pb(12.0).bg_gradient(LinearGradient::scrim(GradientDir::Up, Color::BLACK.alpha(0.85), Color::TRANSPARENT)).child(text("Rodapé").text_color(Color::WHITE)));
    vec![preview(cx, "Lucide", grid), preview(cx, "Na cor do texto", tinted), preview(cx, "Gradiente (scrim)", scrim)]
}
