//! Estado compartilhado pelas telas: sessão, navegação, imagens, AniList em
//! lote, downloads, atualização e avisos. Mora na view raiz.
//!
//! Trabalho assíncrono roda no runtime do núcleo (`AppState::run`) e o resultado
//! volta para a thread da UI pela `Mailbox` da janela (`Post`): nada prende as
//! threads da lib, nem um download de minutos.

mod catalog;
mod covers;
pub mod details;
mod listings;
mod download_tasks;
pub mod home;
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

pub use catalog::{CatalogChoice, CatalogPlace};
pub use listings::Listing;
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
    /// Abrir o player.
    Play(crate::video::PlayTarget),
}

/// Pausa antes de listar a raiz de novo depois de uma falha.
const SECTIONS_RETRY: std::time::Duration = std::time::Duration::from_secs(30);

/// Carga da raiz: uma vez; depois de falhar, só após a pausa (nunca em laço).
#[derive(Default)]
enum SectionsLoad {
    #[default]
    Idle,
    Loading,
    Loaded,
    RetryAt(std::time::Instant),
}

impl SectionsLoad {
    /// Pode pedir agora? Se sim, passa a "carregando".
    fn start(&mut self, now: std::time::Instant) -> bool {
        let go = match self {
            SectionsLoad::Idle => true,
            SectionsLoad::RetryAt(t) => now >= *t,
            SectionsLoad::Loading | SectionsLoad::Loaded => false,
        };
        if go {
            *self = SectionsLoad::Loading;
        }
        go
    }

    fn loaded(&mut self) {
        *self = SectionsLoad::Loaded;
    }

    fn failed(&mut self, now: std::time::Instant) {
        *self = SectionsLoad::RetryAt(now + SECTIONS_RETRY);
    }
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
    /// Respostas de outras threads para o player aberto.
    pub player_inbox: Vec<crate::video::PlayerMsg>,
    sections_load: SectionsLoad,
    images: images::Images,
    /// Leitor aberto (solto ao fechar).
    pub reader: Option<crate::reader::ReaderUi>,
    /// Player de músicas (estado do motor, álbum, tags).
    pub music: crate::music::MusicUi,
    covers: covers::Covers,
    details: details::Details,
    /// Pasta da tela cujos detalhes estão guardados (troca = solta).
    details_root: Option<String>,
    pub(crate) catalog: catalog::CatalogState,
    listings: listings::Listings,
    /// Listas do início (soltas ao sair dele).
    pub home: home::HomeData,
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
            player_inbox: Vec::new(),
            sections_load: SectionsLoad::default(),
            images: images::Images::default(),
            reader: None,
            music: Default::default(),
            covers: covers::Covers::default(),
            details: details::Details::default(),
            details_root: None,
            catalog: catalog::CatalogState::default(),
            listings: listings::Listings::default(),
            home: home::HomeData::default(),
            lookups: lookup::Lookups::default(),
            events: Vec::new(),
        };
        s.load_session();
        s.check_update();
        s
    }

    /// Canal para a thread da UI (player e miniaturas).
    pub fn post(&self) -> Post {
        self.post.clone()
    }

    /// Pede para abrir o player (a raiz cria na próxima renderização).
    pub fn play(&mut self, target: crate::video::PlayTarget) {
        self.events.push(AppEvent::Play(target));
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
        // Só a listagem da tela nova fica na memória.
        // Fanart que falhou tenta de novo na próxima navegação.
        self.covers.retry_failed();
        if !self.path().is_empty() {
            self.home = home::HomeData::default();
        }
        let root = listings::screen_root(self.path());
        self.keep_listings(root.as_deref());
        if root != self.details_root {
            self.forget_details();
            self.details_root = root;
        }
        self.emit(AppEvent::Navigated);
    }

    /// Lista a raiz da nuvem uma vez (só a raiz; nada de varrer pastas).
    /// Falha (rede, 429, sessão) espera `SECTIONS_RETRY` antes de tentar de novo.
    pub fn load_sections(&mut self) {
        if self.user.is_none() || !self.sections_load.start(std::time::Instant::now()) {
            return;
        }
        let core = self.core.clone();
        self.run(async move { core.cloud().list("").await.map(|l| l.files) }, |s, result, _| match result {
            Ok(files) => {
                s.sections_load.loaded();
                let names = files.into_iter().filter(|f| f.is_directory).map(|f| f.name).collect();
                s.sections = crate::chrome::sort_sections(names);
            }
            Err(e) => {
                tracing::warn!(erro = %e, "raiz da nuvem");
                s.sections_load.failed(std::time::Instant::now());
            }
        });
    }

    pub fn refresh_player_data(&mut self) {
        self.player_data = self.core.player().data();
    }
}

#[cfg(test)]
mod sections_tests {
    use super::SectionsLoad;
    use std::time::{Duration, Instant};

    #[test]
    fn raiz_falha_espera_antes_de_tentar_de_novo() {
        let t0 = Instant::now();
        let mut s = SectionsLoad::default();
        assert!(s.start(t0));
        assert!(!s.start(t0), "já carregando");
        s.failed(t0);
        assert!(!s.start(t0 + Duration::from_secs(1)), "não repete na hora");
        assert!(s.start(t0 + Duration::from_secs(31)), "tenta de novo depois da pausa");
        s.loaded();
        assert!(!s.start(t0 + Duration::from_secs(999)), "carregou: nunca mais");
    }
}
