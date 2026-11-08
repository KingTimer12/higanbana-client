//! Acesso autenticado aos serviços do Anitsu, compartilhado pelos módulos.

use crate::{auth::AuthManager, events::EventFn, http::AnitsuApis, nuvem::gate::Gate, paths::Paths};

/// Clientes HTTP + sessão + pastas + eventos. Vive num `Arc` dentro do [`crate::Core`].
pub struct Cloud {
    pub apis: AnitsuApis,
    pub auth: AuthManager,
    pub paths: Paths,
    pub(crate) events: EventFn,
    /// Limitador global de listagem e busca na nuvem.
    pub(crate) gate: Gate,
}

impl Cloud {
    pub fn new(paths: Paths, events: EventFn) -> crate::auth::AuthResult<Self> {
        Ok(Self { apis: AnitsuApis::new(), auth: AuthManager::new(&paths.data, events.clone())?, paths, events, gate: Gate::default() })
    }
}
