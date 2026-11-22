//! Sessão do Discord: lida ao abrir e atualizada pelos eventos do núcleo.

use higanbana_core::auth::Session;

use super::AppState;

#[derive(Clone, Debug)]
pub struct User {
    pub name: String,
    pub avatar: Option<String>,
}

/// Nome de exibição do Discord guardado na sessão.
pub fn user_of(s: &Session) -> User {
    let meta = s.user.as_ref().and_then(|u| u.get("user_metadata"));
    let get = |k: &str| meta.and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(str::to_owned);
    User {
        name: get("full_name").or_else(|| get("name")).or_else(|| get("user_name")).unwrap_or_else(|| "Você".into()),
        avatar: get("avatar_url"),
    }
}

impl AppState {
    pub(super) fn load_session(&mut self) {
        let core = self.core.clone();
        self.run(async move { core.auth().session().await.ok().flatten() }, |s, session, _| {
            s.user = session.as_ref().map(user_of);
            s.session_ready = true;
        });
    }

    pub fn set_session(&mut self, session: Option<&Session>) {
        self.user = session.map(user_of);
        self.session_ready = true;
    }

    /// Sai da conta; o núcleo avisa a troca de sessão (`AuthChanged`).
    pub fn logout(&self) {
        let core = self.core.clone();
        self.core.spawn(async move {
            let _ = core.auth().logout().await;
        });
    }
}
