//! Peças pequenas compartilhadas pelas telas, em cima dos componentes da lib.

pub mod cover;
pub mod download;
pub mod files;
pub mod hover;
pub mod image;
pub mod options;
pub mod paths;

use std::borrow::Cow;

use higanbana_gpui::prelude::*;

/// Ícones Lucide que as telas usam (todos embutidos na lib).
pub const APP_ICONS: &[&str] = &[
    "arrow-down", "arrow-left", "arrow-right", "audio-lines", "book-open", "captions", "check", "chevron-left", "chevron-right",
    "compass", "flame", "sparkles", "trophy", "chevrons-right", "circle-alert", "clapperboard", "columns-2", "copy", "disc-3",
    "download", "external-link", "file-play", "folder", "folder-check", "folder-open", "house", "images", "inbox", "info",
    "list-video", "loader-circle", "log-in", "log-out", "maximize", "minimize", "music", "palette", "pause", "play",
    "rectangle-vertical", "refresh-cw", "repeat", "repeat-1", "scroll-text", "search", "settings-2", "shuffle", "skip-back",
    "skip-forward", "sliders-horizontal", "star", "triangle-alert", "tv", "volume-1", "volume-2", "volume-x", "wifi-off", "x",
    "zoom-in", "zoom-out", "panel-left-open", "panel-left-close", "circle-check", "circle-x", "chevron-down",
];

/// Texto em uma linha só, cortado no fim.
pub fn truncate<V: 'static>(t: impl Into<Cow<'static, str>>) -> Element<V> {
    div().min_w(0.0).overflow_hidden().whitespace_nowrap().child(text(t))
}

/// Bloco de estado vazio ou de erro: ícone, título e explicação.
pub fn empty_state<V: 'static>(theme: &Theme, name: &'static str, title: impl Into<Cow<'static, str>>, detail: Option<Cow<'static, str>>) -> Element<V> {
    let muted = theme.muted_foreground;
    let mut e = div()
        .flex_col()
        .items_center()
        .gap(8.0)
        .py(48.0)
        .px(24.0)
        .child(icon(name).size(32.0).text_color(muted))
        .child(text(title).font_weight(500));
    if let Some(d) = detail {
        e = e.child(div().max_w(384.0).text_size(theme.typography.sm).text_color(muted).child(text(d)));
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icones_do_app_existem() {
        let faltando: Vec<_> = APP_ICONS.iter().filter(|n| !higanbana_gpui::icon::exists(n)).collect();
        assert!(faltando.is_empty(), "ícones sem SVG: {faltando:?}");
    }
}
