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
mod search;
mod session;
mod updates;

use std::future::Future;
use std::sync::Arc;

use higanbana_core::{Core, player::store::PlayerData};
use higanbana_gpui::EventCx;

pub use downloads::{DlStatus, Download, Downloads};
pub use images::Img;
pub use nav::Nav;
pub use search::{kind_heading, Search};
pub use session::User;
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
    pub search: Search,
    /// Seções da raiz da nuvem (barra lateral), em ordem de leitura.
    pub sections: Vec<String>,
    /// Capas da temporada (parede da tela de login).
    pub season: Vec<higanbana_core::anilist::types::MediaSummary>,
    /// Fim do login pelo navegador: `Some(erro)` quando falhou.
    pub login_result: Option<Option<String>>,
    sections_loading: bool,
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
            search: Search::default(),
            sections: Vec::new(),
            season: Vec::new(),
            login_result: None,
            sections_loading: false,
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

    /// Lista a raiz da nuvem uma vez (só a raiz; nada de varrer pastas).
    pub fn load_sections(&mut self) {
        if self.sections_loading || !self.sections.is_empty() || self.user.is_none() {
            return;
        }
        self.sections_loading = true;
        let core = self.core.clone();
        self.run(async move { core.cloud().list("").await.map(|l| l.files).unwrap_or_default() }, |s, files, _| {
            s.sections_loading = false;
            let names = files.into_iter().filter(|f| f.is_directory).map(|f| f.name).collect();
            s.sections = crate::chrome::sort_sections(names);
        });
    }

    pub fn refresh_player_data(&mut self) {
        self.player_data = self.core.player().data();
    }
}
