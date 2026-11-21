//! Ação "Baixar" de um arquivo, mostrando o estado do download no mesmo lugar.

use higanbana_gpui::components::{Button, Component, ComponentSize, Progress, ProgressSize, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use crate::chrome::tip;
use crate::root::Higanbana;
use crate::state::DlStatus;

fn sub(id: ElementId, k: u64) -> ElementId {
    ElementId(id.0.rotate_left(11) ^ k)
}

fn start(path: String) -> impl Fn(&mut Higanbana, &MouseEvent, &mut EventCx) + 'static {
    move |v, _, cx| {
        v.state.start_downloads(std::slice::from_ref(&path));
        cx.notify();
    }
}

fn cancel(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: ElementId, path: &str) -> Element<Higanbana> {
    let path = path.to_owned();
    let b = Button::new(sub(id, 1), "")
        .variant(Variant::Ghost)
        .size(ComponentSize::Xs)
        .icon(icon("x").size(12.0))
        .on_click(move |v: &mut Higanbana, _, cx| {
            if let Some(id) = v.state.downloads.get(&path).map(|d| d.id.clone()) {
                v.state.cancel_download(&id);
            }
            cx.notify();
        })
        .render(cx);
    tip(v, cx, sub(id, 2), b, "Cancelar download")
}

pub fn download_button(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, id: impl Into<ElementId>, path: &str) -> Element<Higanbana> {
    let id = id.into();
    let t = cx.theme().clone();
    // Só estado e fração: o download inteiro não é copiado por linha.
    let d = v.state.downloads.get(path).map(|d| (d.status, d.progress()));
    match d {
        Some((DlStatus::Queued, _)) => {
            let b = Button::new(id, "Na fila").variant(Variant::Ghost).size(ComponentSize::Compact).disabled(true).render(cx);
            div().gap(4.0).items_center().child(b).child(cancel(v, cx, id, path))
        }
        Some((DlStatus::Running, p)) => {
            let bar = Progress::new(sub(id, 3)).value(p * 100.0).size(ProgressSize::Small).render(cx);
            let col = div().w(80.0).flex_col().gap(4.0).items_center().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(format!("{}%", (p * 100.0).round()))).child(div().w_full().child(bar));
            div().gap(4.0).items_center().child(col).child(cancel(v, cx, id, path))
        }
        Some((DlStatus::Done, _)) => div().gap(4.0).px(8.0).items_center().text_size(t.typography.xs).text_color(t.success_foreground).child(icon("check").size(12.0)).child(text("Baixado")),
        Some((DlStatus::Error, _)) => Button::new(id, "Tentar de novo").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("refresh-cw").size(14.0)).on_click(start(path.to_owned())).render(cx),
        None => {
            let b = Button::new(id, "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("download").size(14.0)).on_click(start(path.to_owned())).render(cx);
            tip(v, cx, sub(id, 4), b, "Baixar para o computador")
        }
    }
}
