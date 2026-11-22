//! Capa 2:3 (AniList/fanart ou tipográfica), pôster com título e o card de
//! título da nuvem que pede os metadados quando aparece.

use std::sync::Arc;

use higanbana_core::anilist::{text as anitext, types::MediaSummary, MediaType};
use higanbana_core::library::{folder_title, folder_year};
use higanbana_gpui::components::{Component, Skeleton};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{Corners, ElementId};

use super::hover;
use super::image::{hex_color, remote_image};
use crate::root::Higanbana;
use crate::theme::{media_scrim, on_media};

/// Folga para pedir os metadados antes de o card aparecer.
const LOOKUP_PREFETCH: f32 = 400.0;

/// Colunas que cabem em `width` com itens de pelo menos `min` e espaço `gap`.
pub fn columns(width: f32, min: f32, gap: f32) -> usize {
    (((width + gap) / (min + gap)).floor() as usize).max(1)
}

/// Largura útil do conteúdo: janela − barra lateral − margens (32 de cada lado), no máximo `max`.
pub fn content_width(v: &Higanbana, cx: &RenderCx<'_, Higanbana>, max: f32) -> f32 {
    let sidebar = if v.state.sidebar_collapsed() { 48.0 } else { 255.0 };
    (cx.viewport().width - sidebar - 64.0).clamp(200.0, max)
}

/// Capa tipográfica para títulos sem AniList: o nome inteiro sobre a superfície.
fn type_poster(t: &Theme, name: &str) -> Element<Higanbana> {
    div()
        .size_full()
        .flex_col()
        .justify_between()
        .p(12.0)
        .bg(t.muted)
        .child(icon("clapperboard").size(14.0).text_color(t.muted_foreground))
        .child(div().flex_col().text_size(t.typography.base).font_weight(600).text_color(t.foreground.fade(0.8)).line_clamp(5).child(text(folder_title(name))))
}

/// Capa 2:3 de largura `width` (`None` = largura do pai).
pub fn cover(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, media: Option<&MediaSummary>, name: &str, width: Option<f32>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let bg = media.and_then(|m| hex_color(m.color())).unwrap_or(t.muted);
    let mut base = div().relative().overflow_hidden().rounded(t.radius_lg).bg(bg);
    base = match width {
        Some(w) => base.w(w).h(w * 1.5),
        None => base.w_full(),
    };
    match media.and_then(|m| v.state.cover_key(m, false)) {
        Some(url) => base.child(remote_image(v, cx, Some(&url), t.radius_lg)),
        None => base.child(type_poster(&t, name)),
    }
}

/// Marca de "está no acervo", sobre a capa.
pub fn in_library_mark(t: &Theme) -> Element<Higanbana> {
    div().absolute().top(8.0).left(8.0).w(24.0).h(24.0).rounded_full().items_center().justify_center().bg(t.primary).child(icon("folder-check").size(12.0).text_color(t.primary_foreground))
}

/// Nota do AniList sobre a capa.
fn score_mark(t: &Theme, score: i64) -> Element<Higanbana> {
    div()
        .absolute()
        .top(8.0)
        .right(8.0)
        .items_center()
        .gap(4.0)
        .px(6.0)
        .py(2.0)
        .rounded(t.radius)
        .bg(media_scrim(0.7))
        .text_size(t.typography.xs)
        .font_weight(600)
        .text_color(on_media())
        .child(icon("star").size(12.0).text_color(t.base.yellow))
        .child(text(score.to_string()))
}

pub struct PosterProps {
    pub id: ElementId,
    pub name: String,
    pub media: Option<Arc<MediaSummary>>,
    pub loading: bool,
    pub width: f32,
    pub badge: bool,
    pub dimmed: bool,
    /// Linha de baixo; `None` mostra formato · ano.
    pub meta: Option<String>,
}

/// "Formato · ano" (ou só um deles).
pub fn meta_line(media: Option<&MediaSummary>, name: &str) -> String {
    let year = media.and_then(|m| m.year()).or_else(|| folder_year(name).map(|y| y as i32));
    let format = media.and_then(|m| m.format.as_deref()).map(|f| anitext::format(Some(f))).filter(|f| !f.is_empty());
    match (format, year) {
        (Some(f), Some(y)) => format!("{f} · {y}"),
        (Some(f), None) => f,
        (None, Some(y)) => y.to_string(),
        (None, None) => String::new(),
    }
}

/// Card de pôster (capa + título + formato/ano), com anel no hover. O clique vem de quem usa.
pub fn poster(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, p: PosterProps) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let media = p.media.as_deref();
    let cover_el = if p.loading {
        Skeleton::new(ElementId(p.id.0 ^ 0x5ce1)).w(p.width).h(p.width * 1.5).render(cx).rounded(t.radius_lg)
    } else {
        cover(v, cx, media, &p.name, Some(p.width))
    };
    let meta = p.meta.unwrap_or_else(|| meta_line(media, &p.name));
    let ring = hover::layer(hover::amount(cx, p.id), Corners::all(t.radius_lg), None, Some(t.ring));
    let mut art = div().relative().rounded(t.radius_lg).child(cover_el);
    if p.badge {
        art = art.child(in_library_mark(&t));
    }
    if let Some(s) = media.and_then(|m| m.average_score) {
        art = art.child(score_mark(&t, s));
    }
    let mut info = div().flex_col().gap(2.0).child(div().flex_col().text_size(t.typography.sm).font_weight(600).line_clamp(2).child(text(folder_title(&p.name))));
    if !meta.is_empty() {
        info = info.child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(meta)));
    }
    let card = div().flex_col().gap(8.0).w(p.width).cursor_pointer().opacity(if p.dimmed { 0.55 } else { 1.0 }).child(art.child(ring)).child(info);
    hover::track(card, p.id)
}

/// Pede os metadados de `name` quando o elemento aparecer.
pub fn want_lookup(name: String, media: MediaType) -> Element<Higanbana> {
    on_visible(LOOKUP_PREFETCH, move |v: &mut Higanbana, cx| {
        if v.state.want_lookup(&name, media) {
            cx.notify();
        }
    })
}

/// Pôster de um título da nuvem numa grade: busca os metadados quando aparece e abre o caminho ao clicar.
pub fn title_card(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, prefix: &'static str, name: &str, path: &str, media: MediaType, width: f32) -> Element<Higanbana> {
    let looked = v.state.lookup(name, media);
    let loading = looked.is_none();
    let id = ElementId::from((prefix, path));
    let card = poster(v, cx, PosterProps { id, name: name.to_owned(), media: looked.flatten(), loading, width, badge: false, dimmed: false, meta: None });
    let target = path.to_owned();
    let card = card.on_click(move |v: &mut Higanbana, _, cx| {
        v.state.go(target.clone());
        cx.notify();
    });
    let mut d = div().relative().child(card);
    if loading {
        d = d.child(want_lookup(name.to_owned(), media));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colunas_cabem() {
        assert_eq!(columns(1000.0, 150.0, 16.0), 6);
        assert_eq!(columns(100.0, 150.0, 16.0), 1);
    }

    #[test]
    fn meta_sem_dados_usa_ano_da_pasta() {
        assert_eq!(meta_line(None, "Frieren [2023]"), "2023");
        assert_eq!(meta_line(None, "Sem ano"), "");
    }
}
