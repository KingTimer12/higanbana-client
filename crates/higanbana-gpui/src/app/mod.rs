//! App: janela + view + laço de eventos "retido leve".
mod events;
mod measurer;
mod overlay;
mod requests;
mod run;

pub use measurer::GpuMeasurer;

use crate::cx::RenderCx;
use crate::element::Element;
use crate::error::Result;
use crate::gfx::{BackendKind, WindowOptions};
use crate::theme::Theme;

/// Estado da aplicação que sabe se descrever como árvore de elementos.
pub trait View: Sized + 'static {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self>;
}

/// Configuração do app; `run` bloqueia até a janela fechar.
pub struct App {
    pub(crate) backend: BackendKind,
    pub(crate) theme: Theme,
    pub(crate) show_stats: bool,
    pub(crate) vsync: bool,
}

impl App {
    pub fn new() -> Self {
        Self { backend: BackendKind::Auto, theme: Theme::dark(), show_stats: false, vsync: true }
    }

    /// Backend preferido (cai para outro se indisponível).
    pub fn backend(mut self, kind: BackendKind) -> Self {
        self.backend = kind;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Soma ícones SVG do app aos Lucide embutidos (`icon("nome")`).
    pub fn icons(self, set: crate::icon::IconSet) -> Self {
        crate::icon::register(set);
        self
    }

    /// Desenha backend, tempo de frame e memória no canto da janela.
    pub fn show_stats(mut self, on: bool) -> Self {
        self.show_stats = on;
        self
    }

    pub fn vsync(mut self, on: bool) -> Self {
        self.vsync = on;
        self
    }

    /// Abre a janela e roda até ela fechar (ou `cx.quit()`).
    pub fn run<V: View>(self, options: WindowOptions, view: V) -> Result<()> {
        run::run(self, options, view)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
