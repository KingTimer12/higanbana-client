//! Contextos entregues à view: de render (leitura) e de evento (pedidos ao app).
use crate::gfx::{BackendInfo, BackendKind, Stats};
use crate::theme::Theme;
use std::marker::PhantomData;

/// Contexto de `View::render`: tema, backends e estatísticas.
pub struct RenderCx<'a, V: ?Sized> {
    pub(crate) theme: &'a Theme,
    pub(crate) backend: BackendKind,
    pub(crate) backends: &'a [BackendInfo],
    pub(crate) stats: Stats,
    pub(crate) _view: PhantomData<fn(&mut V)>,
}

impl<'a, V: ?Sized> RenderCx<'a, V> {
    pub fn new(theme: &'a Theme, backend: BackendKind, backends: &'a [BackendInfo], stats: Stats) -> Self {
        Self { theme, backend, backends, stats, _view: PhantomData }
    }

    pub fn theme(&self) -> &'a Theme {
        self.theme
    }

    /// Backend ativo.
    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    /// Backends disponíveis nesta máquina.
    pub fn backends(&self) -> &'a [BackendInfo] {
        self.backends
    }

    /// Estatísticas do último frame.
    pub fn stats(&self) -> Stats {
        self.stats
    }
}

/// Contexto de handlers: marca a view suja e enfileira pedidos ao app.
#[derive(Default)]
pub struct EventCx {
    pub(crate) notified: bool,
    pub(crate) switch_to: Option<BackendKind>,
    pub(crate) theme: Option<Theme>,
    pub(crate) quit: bool,
}

impl EventCx {
    /// O estado mudou: reconstrói a árvore no próximo frame.
    pub fn notify(&mut self) {
        self.notified = true;
    }

    /// Troca o backend de GPU depois do handler (sem fechar a janela).
    pub fn switch_backend(&mut self, kind: BackendKind) {
        self.switch_to = Some(kind);
        self.notified = true;
    }

    /// Substitui o tema do app.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
        self.notified = true;
    }

    /// Fecha a janela e encerra `App::run`.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    pub fn is_notified(&self) -> bool {
        self.notified
    }
}
