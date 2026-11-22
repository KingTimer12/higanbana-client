//! Índice: capítulos/volumes com busca; o atual realçado, os lidos com ✓.

use higanbana_core::reader::logic::chapter_label;
use std::sync::Arc;

use higanbana_gpui::components::{Component, Input, VirtualList};
use higanbana_gpui::prelude::*;

use super::{card, count_word, heading, index_items};
use crate::root::Higanbana;

/// Altura de cada linha e teto da lista.
const ROW: f32 = 41.0;
const LIST_MAX: f32 = 400.0;

pub fn panel(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let chapters = r.chapters.clone();
    let (n, word) = (chapters.len(), count_word(&chapters));
    let current = r.chapter();
    let items = index_items(&chapters, r.filter.text());
    let filter = Input::new("leitor-filtro", &r.filter, |v: &mut Higanbana| &mut v.state.reader.as_mut().expect("painel com leitor").filter)
        .kit()
        .prefix("search")
        .height(28.0)
        .on_change(|_: &mut Higanbana, _, cx| cx.notify())
        .render(cx);
    let progress = r.progress.clone();
    let row = move |cx: &mut RenderCx<'_, Higanbana>, i: usize| -> Element<Higanbana> {
        let t = cx.theme().clone();
        let u = &chapters[i];
        let l = chapter_label(&u.name);
        let here = i == current;
        let read = progress.get(&u.key).is_some_and(|p| p.read);
        let hover = t.list_hover;
        let mut row = div().id(("capitulo", u.key.as_str())).h(ROW - 1.0).items_center().gap(8.0).px(8.0).rounded(t.radius).cursor_pointer();
        row = if here { row.bg(t.list_active) } else { row.hover(move |s| s.bg(hover)) };
        let name = match &l.number {
            Some(num) => div()
                .flex_1()
                .items_baseline()
                .gap(6.0)
                .child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(l.kind)))
                .child(text(num.clone()).text_size(t.typography.lg).font_weight(700).text_color(if here { crate::theme::primary_ink(&t) } else { t.foreground })),
            None => div().flex_1().min_w(0.0).text_size(t.typography.sm).child(text(l.name.clone().unwrap_or_default()).line_clamp(1)),
        };
        row = row.child(name);
        if read {
            row = row.child(div().items_center().gap(4.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(icon("check").size(12.0)).child(text("lido")));
        }
        row.on_click(move |v: &mut Higanbana, _, cx| {
            if let Some(r) = v.state.reader.as_mut() {
                r.resume_chapter(i);
            }
            v.state.pump_reader();
            cx.notify();
        })
    };
    let list = if items.is_empty() {
        div().px(16.0).py(24.0).text_size(t.typography.sm).text_color(t.muted_foreground).child(text("Nada com esse nome."))
    } else {
        // Só as linhas visíveis são montadas (pastas com centenas de capítulos).
        let heights: Arc<[f32]> = vec![ROW; items.len()].into();
        let h = (ROW * items.len() as f32).min(LIST_MAX);
        let body = VirtualList::new("leitor-indice", heights).render(cx, |k, cx| row(cx, items[k]));
        div().h(h).px(8.0).child(body)
    };
    let count = format!("{} {}", n, word);
    let head = div().mx(16.0).mb(8.0).items_baseline().justify_between().child(heading(cx, "Índice").mb(0.0)).child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(count)));
    card(cx, "leitor-painel-indice").bottom(48.0).left(16.0).w(304.0).max_h(512.0).py(12.0).child(head).child(div().mx(16.0).mb(8.0).child(filter)).child(list)
}
