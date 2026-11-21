//! Estados sobre a área do vídeo: carregando, mpv quebrado, erro fatal e sem rede.

use higanbana_core::player::logic::episode_label;
use higanbana_gpui::components::{Button, Component, ComponentSize, Spinner, Variant};
use higanbana_gpui::prelude::*;

use super::style::media_card;
use super::PlayerView;
use crate::root::Higanbana;
use crate::theme::on_media_muted;

/// O que mostrar no meio do vídeo.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    None,
    /// Esperando o cache; `opening` = antes do primeiro quadro.
    Buffering { opening: bool },
    Broken(String),
    Fatal,
    NetError,
}

/// Prioridade da master: quebrado > fatal > sem rede > carregando.
pub fn overlay(p: &PlayerView) -> Overlay {
    if let Some(e) = &p.broken {
        return Overlay::Broken(e.clone());
    }
    if p.session.fatal() {
        return Overlay::Fatal;
    }
    if p.session.net_error() {
        return Overlay::NetError;
    }
    let st = &p.last;
    let loading = matches!(st.status, higanbana_gpui::video::Status::Loading | higanbana_gpui::video::Status::Idle);
    if st.buffering || loading {
        return Overlay::Buffering { opening: !p.video_ready() };
    }
    Overlay::None
}

fn centered(card: Element<Higanbana>) -> Element<Higanbana> {
    div().absolute().inset(0.0).justify_center().child(card)
}

pub fn status(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Option<Element<Higanbana>> {
    let p = v.player.as_ref()?;
    let ep = episode_label(&p.session.target.file.name);
    let what = overlay(p);
    let t = cx.theme().clone();
    let (sm, xs) = (t.typography.sm, t.typography.xs);
    let danger = t.destructive_foreground;
    Some(match what {
        Overlay::None => return None,
        Overlay::Buffering { opening } => {
            let spin = Spinner::new("player-carregando").size(24.0).color(on_media_muted()).render(cx);
            div()
                .absolute()
                .inset(0.0)
                .flex_col()
                .justify_center()
                .items_center()
                .gap(12.0)
                .child(spin)
                .when(opening, |d| d.child(div().text_size(sm).text_color(on_media_muted()).child(text(format!("Abrindo {ep}…")))))
        }
        Overlay::Broken(msg) => centered(
            media_card(&t)
                .items_center()
                .text_size(sm)
                .child(icon("circle-alert").size(16.0).text_color(danger))
                .child(text("O motor de vídeo (mpv) não abriu."))
                .child(div().text_size(xs).text_color(t.muted_foreground).child(text(msg))),
        ),
        Overlay::Fatal => {
            let retry = Button::new("player-tentar", "Tentar de novo")
                .variant(Variant::Default)
                .size(ComponentSize::Compact)
                .icon(icon("refresh-cw").size(14.0))
                .on_click(|v: &mut Higanbana, _, cx| {
                    cx.stop_propagation();
                    if let Some(p) = v.player.as_mut() {
                        let a = p.session.retry();
                        p.run(vec![a]);
                    }
                    cx.notify();
                })
                .render(cx);
            centered(
                media_card(&t)
                    .items_center()
                    .text_size(sm)
                    .child(icon("circle-alert").size(16.0).text_color(danger))
                    .child(div().font_weight(600).child(text("Não foi possível tocar este arquivo")))
                    .child(
                        div()
                            .max_w(288.0)
                            .justify_center()
                            .text_size(xs)
                            .text_color(t.muted_foreground)
                            .child(text("Pode ser a conexão, a sessão expirada ou um arquivo indisponível na nuvem.")),
                    )
                    .child(retry),
            )
        }
        Overlay::NetError => centered(
            media_card(&t)
                .flex_row()
                .items_center()
                .gap(12.0)
                .text_size(sm)
                .child(icon("wifi-off").size(16.0).text_color(danger))
                .child(text("Sem conexão com a nuvem. Tentando de novo…")),
        ),
    })
}
