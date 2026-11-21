//! Raiz da janela: barra de título + barra lateral + tela atual, ou o
//! player/leitor ocupando a janela inteira.

use std::sync::Arc;

use higanbana_core::{Core, Event};
use higanbana_gpui::prelude::*;

/// Estado inteiro do app (a lib não guarda estado; tudo mora aqui).
pub struct Higanbana {
    pub core: Arc<Core>,
}

impl Higanbana {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    /// Evento do núcleo vindo de outra thread (pela `Mailbox`).
    pub fn on_core_event(&mut self, _e: Event, cx: &mut EventCx) {
        cx.notify();
    }
}

impl View for Higanbana {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        let t = cx.theme();
        div().size_full().flex_col().bg(t.background).text_color(t.foreground)
    }
}
