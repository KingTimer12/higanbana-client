//! O que o núcleo avisa à interface sem ela ter pedido.

use std::sync::Arc;

#[cfg(feature = "music")]
use crate::music::engine::PlayerState;
use crate::{auth::Session, nuvem::download::DownloadProgress};

#[derive(Debug, Clone)]
pub enum Event {
    /// A sessão mudou (login, renovação, logout). `None` = deslogado.
    AuthChanged(Option<Session>),
    /// O navegador voltou do login: hora de trazer a janela para a frente.
    LoginReturned,
    /// Progresso de um download (a cada 250 ms e no fim).
    DownloadProgress(DownloadProgress),
    /// Estado do player de música (a cada 250 ms tocando, ou quando muda).
    #[cfg(feature = "music")]
    MusicState(PlayerState),
}

/// Recebe os eventos. Chamado de threads do núcleo: não pode travar.
pub type EventFn = Arc<dyn Fn(Event) + Send + Sync>;

/// Descarta tudo (testes, ferramentas de linha de comando).
pub fn ignore() -> EventFn {
    Arc::new(|_| {})
}
