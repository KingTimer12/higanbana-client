//! Cartão de fim de arquivo e aviso de arquivo que não abriu (ou abriu vazio).

use higanbana_core::reader::logic::{chapter_label, label_text, ReadMode};
use higanbana_gpui::components::{Button, Component, Variant};
use higanbana_gpui::prelude::*;

use super::close;
use crate::root::Higanbana;
use crate::state::DlStatus;

/// Fim do capítulo `ch`, com o próximo (ou o fim da pasta).
pub fn chapter_end(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, ch: usize) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let label = |i: usize| label_text(&chapter_label(&r.chapters.get(i).map(|u| u.name.clone()).unwrap_or_default()));
    let next = ch + 1;
    let mut card = div().flex_col().items_center().justify_center().gap(16.0).p(40.0).child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(format!("Fim de {}", label(ch)))));
    if r.prefs.mode == ReadMode::Wide {
        card = card.h_full().w(320.0).flex_none();
    }
    if next < r.chapters.len() {
        let name = label(next);
        let read = Button::new(("ler-proximo", ch), format!("Ler {name}")).on_click(move |v: &mut Higanbana, _, cx| {
            cx.stop_propagation();
            if let Some(r) = v.state.reader.as_mut() {
                r.resume_chapter(next);
            }
            v.state.pump_reader();
            cx.notify();
        });
        let mut row = div().gap(8.0).child(read.render(cx));
        if r.end {
            let stay = Button::new("ficar-aqui", "Ficar aqui").variant(Variant::Outline).on_click(|v: &mut Higanbana, _, cx| {
                cx.stop_propagation();
                if let Some(r) = v.state.reader.as_mut() {
                    r.end = false;
                }
                cx.notify();
            });
            row = row.child(stay.render(cx));
        }
        card.child(text(name).text_size(30.0).font_weight(700)).child(row)
    } else {
        let back = Button::new(("voltar-titulo", ch), "Voltar ao título").on_click(|v: &mut Higanbana, _, cx| {
            cx.stop_propagation();
            close(v, cx);
        });
        card.child(text("Último arquivo da pasta.").text_size(t.typography.xl).font_weight(700)).child(back.render(cx))
    }
}

/// Arquivo que não abriu (ou abriu vazio): explicação, tentar de novo e baixar.
/// `ch`: capítulo que falhou (na faixa pode não ser o atual).
pub fn open_failed(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, ch: usize, msg: String) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let path = v.state.reader.as_ref().and_then(|r| r.chapters.get(ch).cloned()).map(|u| u.path);
    let download = match path.as_deref().and_then(|p| v.state.downloads.get(p)).map(|d| d.status) {
        Some(DlStatus::Done) => "Baixado",
        Some(DlStatus::Running | DlStatus::Queued) => "Baixando…",
        _ => "Baixar",
    };
    let retry = Button::new(("tentar-abrir", ch), "Tentar de novo").on_click(move |v: &mut Higanbana, _, cx| {
        cx.stop_propagation();
        if let Some(r) = v.state.reader.as_mut() {
            r.retry_chapter(ch);
        }
        v.state.pump_reader();
        cx.notify();
    });
    let get = Button::new(("baixar-arquivo", ch), download).variant(Variant::Outline).icon(icon("download").size(16.0)).on_click(move |v: &mut Higanbana, _, cx| {
        cx.stop_propagation();
        if let Some(p) = path.clone() {
            v.state.start_downloads(&[p]);
        }
        cx.notify();
    });
    let body = div()
        .max_w(384.0)
        .flex_col()
        .items_center()
        .gap(12.0)
        .child(text("Não deu para abrir este arquivo.").text_size(t.typography.lg).font_weight(700))
        .child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(msg)))
        .child(div().mt(8.0).gap(8.0).child(retry.render(cx)).child(get.render(cx)));
    div().size_full().flex_col().items_center().justify_center().child(body)
}
