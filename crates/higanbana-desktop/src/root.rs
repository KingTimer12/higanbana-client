//! Raiz da janela: barra de título + barra lateral + tela atual, ou o
//! player/leitor ocupando a janela inteira.

use std::sync::Arc;

use higanbana_core::{Core, Event};
use higanbana_gpui::prelude::*;

use crate::state::{AppState, Post};

/// Estado inteiro do app (a lib não guarda estado; tudo mora aqui).
pub struct Higanbana {
    pub state: AppState,
}

impl Higanbana {
    pub fn new(core: Arc<Core>, post: Post) -> Self {
        Self { state: AppState::new(core, post) }
    }

    /// Evento do núcleo vindo de outra thread (pela `Mailbox`).
    pub fn on_core_event(&mut self, e: Event, cx: &mut EventCx) {
        match &e {
            Event::AuthChanged(session) => self.state.set_session(session.as_ref()),
            Event::DownloadProgress(p) => self.state.on_download_progress(p),
            // Player de músicas e volta do login pelo navegador: Plano 4.
            Event::MusicState(_) | Event::LoginReturned => return,
        }
        cx.notify();
    }
}

impl View for Higanbana {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        let t = cx.theme();
        div().size_full().flex_col().bg(t.background).text_color(t.foreground)
    }
}
