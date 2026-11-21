//! Avisos curtos e preferências da moldura (barra lateral recolhida).

use super::{AppEvent, AppState, Tone};

impl AppState {
    pub fn toast(&mut self, message: impl Into<String>, tone: Tone) {
        self.emit(AppEvent::Notify(tone, message.into()));
    }

    /// Barra lateral só com ícones; guardada nas configurações.
    pub fn sidebar_collapsed(&self) -> bool {
        self.core.settings().sidebar_collapsed()
    }

    pub fn toggle_sidebar(&mut self) {
        let collapsed = !self.sidebar_collapsed();
        self.core.settings().set_sidebar_collapsed(collapsed);
    }
}
