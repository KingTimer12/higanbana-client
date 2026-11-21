//! Estado compartilhado pelas telas: sessão, navegação, imagens, AniList em
//! lote, downloads, atualização e avisos. Mora na view raiz.
//!
//! Trabalho assíncrono roda no runtime do núcleo (`AppState::run`) e o resultado
//! volta para a thread da UI pela `Mailbox` da janela (`Post`): nada prende as
//! threads da lib, nem um download de minutos.

mod download_tasks;
mod downloads;
mod images;
mod lookup;
mod nav;
mod notify;
mod session;
mod updates;

use std::future::Future;
use std::sync::Arc;

use higanbana_core::{Core, player::store::PlayerData};
use higanbana_gpui::EventCx;

pub use downloads::{DlStatus, Download, Downloads};
pub use images::Img;
pub use nav::Nav;
pub use session::{User, user_of};
pub use updates::UpdateState;

/// Trabalho que volta à UI com o estado e o contexto de evento.
pub type Job = Box<dyn FnOnce(&mut AppState, &mut EventCx) + Send>;
/// Manda um `Job` para a thread da UI (a `Mailbox` da janela, em `main`).
pub type Post = Arc<dyn Fn(Job) + Send + Sync>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Ok,
    Error,
}

/// Pedidos que a raiz atende depois de um evento (drenados a cada render).
#[derive(Debug, PartialEq)]
pub enum AppEvent {
    Navigated,
    /// Aviso curto para o usuário (vira notificação na janela).
    Notify(Tone, String),
}

pub struct AppState {
    pub core: Arc<Core>,
    post: Post,
    /// `false` antes de ler a sessão salva.
    pub session_ready: bool,
    pub user: Option<User>,
    pub nav: Nav,
    pub downloads: Downloads,
    pub update: UpdateState,
    pub player_data: PlayerData,
    images: images::Images,
    lookups: lookup::Lookups,
    events: Vec<AppEvent>,
}

impl AppState {
    pub fn new(core: Arc<Core>, post: Post) -> Self {
        let player_data = core.player().data();
        let mut s = AppState {
            core,
            post,
            session_ready: false,
            user: None,
            nav: Nav::default(),
            downloads: Downloads::default(),
            update: UpdateState::default(),
            player_data,
            images: images::Images::default(),
            lookups: lookup::Lookups::default(),
            events: Vec::new(),
        };
        s.load_session();
        s.check_update();
        s
    }

    /// Roda `fut` no runtime do núcleo; `done` roda depois na UI.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
        done: impl FnOnce(&mut AppState, T, &mut EventCx) + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let post = self.post.clone();
        self.core.spawn(async move {
            let out = fut.await;
            post(Box::new(move |s, cx| done(s, out, cx)));
        })
    }

    /// Roda `f` na UI depois de `delay` (debounce, nova tentativa).
    pub(crate) fn after(&self, delay: std::time::Duration, f: impl FnOnce(&mut AppState, &mut EventCx) + Send + 'static) -> tokio::task::JoinHandle<()> {
        // O `sleep` nasce dentro do runtime (criado fora dele, entra em pânico).
        self.run(async move { tokio::time::sleep(delay).await }, move |s, (), cx| f(s, cx))
    }

    pub(crate) fn emit(&mut self, e: AppEvent) {
        self.events.push(e);
    }

    /// Eventos para a raiz (navegou, avisos).
    pub fn take_events(&mut self) -> Vec<AppEvent> {
        std::mem::take(&mut self.events)
    }

    // --- Navegação ---

    pub fn path(&self) -> &str {
        self.nav.path()
    }

    pub fn go(&mut self, path: impl Into<String>) {
        if self.nav.go(path) {
            self.navigated();
        }
    }

    pub fn back(&mut self) {
        if self.nav.back() {
            self.navigated();
        }
    }

    pub fn forward(&mut self) {
        if self.nav.forward() {
            self.navigated();
        }
    }

    fn navigated(&mut self) {
        self.emit(AppEvent::Navigated);
    }

    pub fn refresh_player_data(&mut self) {
        self.player_data = self.core.player().data();
    }
}
