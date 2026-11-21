//! Downloads: botão na barra de título com o painel da fila num popover.

use higanbana_gpui::components::{Button, Component, ComponentSize, Popover, PopoverAnchor, Progress, ProgressSize, Variant};
use higanbana_gpui::prelude::*;

use super::{format_bytes, tip};
use crate::root::Higanbana;
use crate::state::{DlStatus, Download};
use crate::ui::{empty_state, truncate};

fn status_text(d: &Download) -> String {
    match d.status {
        DlStatus::Queued => "Na fila".to_owned(),
        DlStatus::Running => match d.total {
            Some(t) => format!("{} de {}", format_bytes(d.downloaded), format_bytes(t)),
            None => format_bytes(d.downloaded),
        },
        DlStatus::Done => "Concluído".into(),
        DlStatus::Error => d.error.clone().unwrap_or_else(|| "Falhou".into()),
    }
}

/// Mostra o arquivo no Explorer.
fn reveal(path: &std::path::Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(not(windows))]
    let _ = path;
}

fn action(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, d: &Download) -> Element<Higanbana> {
    let small = |id: String, name: &'static str| Button::new(id, "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon(name).size(14.0));
    let (b, hint) = match d.status {
        DlStatus::Done => {
            let Some(dest) = d.dest.clone() else { return div() };
            (small(format!("reveal:{}", d.id), "folder-open").on_click(move |_: &mut Higanbana, _, _| reveal(&dest)), "Mostrar na pasta")
        }
        DlStatus::Error => {
            let path = d.path.clone();
            let b = small(format!("retry:{}", d.id), "refresh-cw").on_click(move |v: &mut Higanbana, _, cx| {
                v.state.start_downloads(std::slice::from_ref(&path));
                cx.notify();
            });
            (b, "Tentar de novo")
        }
        DlStatus::Running | DlStatus::Queued => {
            let id = d.id.clone();
            let b = small(format!("cancel:{}", d.id), "x").on_click(move |v: &mut Higanbana, _, cx| {
                v.state.cancel_download(&id);
                cx.notify();
            });
            (b, "Cancelar download")
        }
    };
    let b = b.render(cx);
    tip(v, cx, format!("dica:{}", d.id).as_str(), b, hint)
}

fn row(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, d: &Download) -> Element<Higanbana> {
    let t = cx.theme();
    let (name, color) = match d.status {
        DlStatus::Done => ("check", t.success_foreground),
        DlStatus::Error => ("triangle-alert", t.destructive_foreground),
        _ => ("download", t.muted_foreground),
    };
    let status_color = if d.status == DlStatus::Error { t.destructive_foreground } else { t.muted_foreground };
    let (sm, xs) = (t.typography.sm, t.typography.xs);
    let mut info = div()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .gap(4.0)
        .child(truncate(d.name.clone()).text_size(sm).font_weight(500));
    if d.active() {
        info = info.child(Progress::new(format!("dl-progress:{}", d.id).as_str()).size(ProgressSize::XSmall).value(d.progress() * 100.0).render(cx));
    }
    info = info.child(div().text_size(xs).text_color(status_color).child(text(status_text(d))));
    div()
        .items_center()
        .gap(12.0)
        .px(12.0)
        .py(8.0)
        .child(icon(name).size(14.0).text_color(color))
        .child(info)
        .child(action(v, cx, d))
}

fn panel(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let items = v.state.downloads.items.clone();
    let any_finished = items.iter().any(|d| matches!(d.status, DlStatus::Done | DlStatus::Error));
    let body = if items.is_empty() {
        empty_state(cx.theme(), "download", "Nenhum download ainda", Some("Abra um título e escolha os episódios para baixar.".into()))
    } else {
        let mut list = scroll("downloads-list").flex_col().max_h(384.0);
        for d in &items {
            list = list.child(row(v, cx, d));
        }
        list
    };
    let mut head = div().items_center().justify_between().px(12.0).pb(8.0).child(text("Downloads").font_weight(600));
    if any_finished {
        let clear = Button::new("clear-downloads", "Limpar concluídos").variant(Variant::Ghost).size(ComponentSize::Xs).on_click(|v: &mut Higanbana, _, cx| {
            v.state.clear_finished();
            cx.notify();
        });
        head = head.child(clear.render(cx));
    }
    let border = cx.theme().border;
    div()
        .flex_col()
        .w(384.0)
        .child(head)
        .child(div().h(1.0).w_full().bg(border))
        .child(div().pt(4.0).child(body))
}

pub fn menu(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let (pending, overall) = (v.state.downloads.pending(), v.state.downloads.overall());
    let label = if pending > 0 { pending.to_string() } else { String::new() };
    let trigger = Button::new("downloads", label).variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("download").size(14.0)).render(cx);
    let hint = if pending > 0 { format!("Downloads: {pending} em andamento ({}%)", (overall * 100.).round()) } else { "Downloads".into() };
    let trigger = tip(v, cx, "dica-downloads", trigger, hint);
    let content = panel(v, cx);
    Popover::new("downloads-popover", &v.dl_pop, |v: &mut Higanbana| &mut v.dl_pop, trigger)
        .anchor(PopoverAnchor::TopRight)
        .content(move |_| content)
        .render(cx)
}
