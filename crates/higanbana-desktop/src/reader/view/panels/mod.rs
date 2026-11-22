//! Painéis flutuantes do leitor (índice, páginas, opções): cartão com a superfície
//! dos popovers e fade curto ao abrir.

mod index;
mod options;
mod pages;

use std::collections::HashSet;

use higanbana_core::reader::logic::{chapter_label, label_text, Unit};
use higanbana_gpui::prelude::*;

use crate::motion;
use crate::reader::state::Panel;
use crate::root::Higanbana;

/// Índices dos capítulos que batem com a busca (nome ou rótulo "Cap. 12").
pub fn index_items(chapters: &[Unit], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    chapters
        .iter()
        .enumerate()
        .filter(|(_, u)| q.is_empty() || u.name.to_lowercase().contains(&q) || label_text(&chapter_label(&u.name)).to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

/// "volume(s)", "capítulo(s)" quando todos são do mesmo tipo; senão "arquivo(s)".
pub fn count_word(chapters: &[Unit]) -> &'static str {
    let kinds: HashSet<&str> = chapters.iter().map(|u| chapter_label(&u.name)).filter(|l| l.number.is_some()).map(|l| l.kind).collect();
    let one = chapters.len() == 1;
    match kinds.iter().next().copied().filter(|_| kinds.len() == 1) {
        Some("Vol.") => if one { "volume" } else { "volumes" },
        Some("Cap.") => if one { "capítulo" } else { "capítulos" },
        _ => if one { "arquivo" } else { "arquivos" },
    }
}

/// Cartão do painel; cliques dentro não chegam à área da página.
fn card(cx: &mut RenderCx<'_, Higanbana>, id: &'static str) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let fade = cx.animate_from(id, 0.0, 1.0, motion::transition(motion::QUICK));
    div()
        .id(id)
        .absolute()
        .flex_col()
        .opacity(fade)
        .bg(t.popover)
        .text_color(t.popover_foreground)
        .border(1.0, t.border)
        .rounded(t.radius_lg)
        .shadow(Color::BLACK.fade(0.25), 16.0)
        .on_click(|_: &mut Higanbana, _, cx| cx.stop_propagation())
}

fn heading(cx: &mut RenderCx<'_, Higanbana>, s: &'static str) -> Element<Higanbana> {
    let t = cx.theme();
    div().mb(8.0).text_size(t.typography.sm).font_weight(600).text_color(t.foreground).child(text(s))
}

/// Painel aberto, se houver.
pub fn panel(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Option<Element<Higanbana>> {
    let r = v.state.reader.as_ref()?;
    match r.panel? {
        Panel::Index => Some(index::panel(v, cx)),
        Panel::Pages if r.total() > 0 => Some(pages::panel(v, cx)),
        Panel::Pages => None,
        Panel::Options => Some(options::panel(v, cx)),
    }
}
