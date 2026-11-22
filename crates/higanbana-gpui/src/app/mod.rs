//! App: janela + view + laço de eventos "retido leve".
mod events;
mod fonts;
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
    /// `Inbox<V>` criado por `mailbox` (tipo apagado até o `run`).
    pub(crate) inbox: Option<Box<dyn std::any::Any>>,
    /// Chamado uma vez com o handle nativo logo depois de a janela abrir.
    pub(crate) on_window: Option<Box<dyn FnOnce(usize)>>,
    /// Pastas com .ttf/.otf somadas ao abrir (`fonts_dir`).
    pub(crate) font_dirs: Vec<std::path::PathBuf>,
}

impl App {
    pub fn new() -> Self {
        Self { backend: BackendKind::Auto, theme: Theme::dark(), show_stats: false, vsync: true, inbox: None, on_window: None, font_dirs: Vec::new() }
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

    /// Pasta com fontes (.ttf/.otf) que o app usa sem instalar no Windows.
    /// Pasta ausente é ignorada; se a família do tema faltar, cai no Segoe UI.
    pub fn fonts_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.font_dirs.push(dir.into());
        self
    }

    /// Caixa de correio para mandar trabalho de outras threads à view `V`
    /// (a mesma passada a `run`). Chamar antes de `run`.
    pub fn mailbox<V: View>(&mut self) -> crate::cx::Mailbox<V> {
        let (tx, rx) = crate::cx::inbox::<V>();
        self.inbox = Some(Box::new(rx));
        tx
    }

    /// `f(handle)` roda uma vez com o handle nativo (HWND) da janela aberta.
    pub fn on_window(mut self, f: impl FnOnce(usize) + 'static) -> Self {
        self.on_window = Some(Box::new(f));
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
