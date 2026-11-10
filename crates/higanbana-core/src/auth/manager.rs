use std::path::Path;

use tokio::sync::{Mutex, oneshot};

use super::{
    browser_login::{self, PendingLogin},
    config::CONFIG,
    cookie,
    error::{AuthError, AuthResult},
    gotrue::GoTrueClient,
    session::Session,
    store::SessionStore,
};
use crate::events::{Event, EventFn};

/// Dono único da sessão. Avisa [`Event::AuthChanged`] sempre que ela muda.
///
/// O `Mutex` da sessão fica travado durante o refresh, então chamadas
/// concorrentes esperam e reaproveitam o token novo (um só refresh por vez).
pub struct AuthManager {
    events: EventFn,
    store: SessionStore,
    gotrue: GoTrueClient,
    session: Mutex<Option<Session>>,
    login_lock: Mutex<()>,
    pending_login: Mutex<Option<PendingLogin>>,
}

impl AuthManager {
    pub fn new(data_dir: &Path, events: EventFn) -> AuthResult<Self> {
        let store = SessionStore::new(data_dir)?;
        let session = store.load();
        Ok(Self {
            events,
            store,
            gotrue: GoTrueClient::new(),
            session: Mutex::new(session),
            login_lock: Mutex::new(()),
            pending_login: Mutex::new(None),
        })
    }

    /// Sessão válida (renova se estiver para expirar). `None` se não logado.
    pub async fn session(&self) -> AuthResult<Option<Session>> {
        let mut guard = self.session.lock().await;
        match guard.as_ref() {
            Some(s) if s.needs_refresh() => {
                let refresh_token = s.refresh_token.clone();
                self.refresh_locked(&mut guard, &refresh_token).await.map(Some)
            }
            other => Ok(other.cloned()),
        }
    }

    pub async fn access_token(&self) -> AuthResult<Option<String>> {
        Ok(self.session().await?.map(|s| s.access_token))
    }

    /// Header `Cookie` com a sessão, no mesmo formato que o navegador manda para `*.anitsu.moe`.
    pub async fn cookie_header(&self) -> AuthResult<Option<String>> {
        let Some(session) = self.session().await? else {
            return Ok(None);
        };
        Ok(Some(cookie::encode_session(&CONFIG.session_cookie, &session)?))
    }

    pub async fn is_logged_in(&self) -> bool {
        self.session.lock().await.is_some()
    }

    /// Força refresh (ex.: API respondeu 401).
    pub async fn refresh(&self) -> AuthResult<Session> {
        let mut guard = self.session.lock().await;
        let refresh_token = guard
            .as_ref()
            .ok_or(AuthError::NotAuthenticated)?
            .refresh_token
            .clone();
        self.refresh_locked(&mut guard, &refresh_token).await
    }

    /// Login pelo navegador padrão (Discord já logado lá). Termina quando o
    /// navegador volta para o servidor loopback, ou com cancelamento/timeout.
    pub async fn login(&self) -> AuthResult<Session> {
        self.login_with(browser_login::open_browser).await
    }

    /// Como [`AuthManager::login`], mas quem abre a página é `open` (no celular
    /// a interface abre a URL pelo sistema; ver [`AuthManager::pending_login_url`]).
    pub async fn login_with(&self, open: impl FnOnce(&str) -> AuthResult<()>) -> AuthResult<Session> {
        let _login = self.login_lock.try_lock().map_err(|_| AuthError::InProgress)?;
        let (listener, pkce, url) = browser_login::start(&self.gotrue).await?;
        let (cancel_tx, cancel_rx) = oneshot::channel();
        *self.pending_login.lock().await = Some(PendingLogin { url: url.clone(), cancel: cancel_tx });
        let result = match open(&url) {
            Ok(()) => browser_login::finish(&self.events, &self.gotrue, listener, pkce, cancel_rx).await,
            Err(e) => Err(e),
        };
        self.pending_login.lock().await.take();
        let session = result?;
        let mut guard = self.session.lock().await;
        self.set_locked(&mut guard, Some(session.clone()))?;
        Ok(session)
    }

    /// URL de autorização do login em andamento; `None` fora de um login.
    pub async fn pending_login_url(&self) -> Option<String> {
        self.pending_login.lock().await.as_ref().map(|p| p.url.clone())
    }

    /// Abre de novo a página de login (ex.: a aba foi fechada sem querer).
    pub async fn reopen_login(&self) -> AuthResult<()> {
        let url = self
            .pending_login
            .lock()
            .await
            .as_ref()
            .map(|p| p.url.clone())
            .ok_or(AuthError::NotAuthenticated)?;
        browser_login::open_browser(&url)
    }

    pub async fn cancel_login(&self) {
        if let Some(p) = self.pending_login.lock().await.take() {
            let _ = p.cancel.send(());
        }
    }

    pub async fn logout(&self) -> AuthResult<()> {
        let mut guard = self.session.lock().await;
        if let Some(s) = guard.as_ref() {
            // Melhor esforço: sessão local sai mesmo se o servidor falhar.
            let _ = self.gotrue.logout(&s.access_token).await;
        }
        self.set_locked(&mut guard, None)
    }

    async fn refresh_locked(
        &self,
        guard: &mut Option<Session>,
        refresh_token: &str,
    ) -> AuthResult<Session> {
        match self.gotrue.refresh(refresh_token).await {
            Ok(new) => {
                self.set_locked(guard, Some(new.clone()))?;
                Ok(new)
            }
            Err(e @ AuthError::InvalidSession(_)) => {
                self.set_locked(guard, None)?;
                Err(e)
            }
            // Erro de rede: mantém a sessão para tentar de novo depois.
            Err(e) => Err(e),
        }
    }

    fn set_locked(&self, guard: &mut Option<Session>, session: Option<Session>) -> AuthResult<()> {
        match &session {
            Some(s) => self.store.save(s)?,
            None => self.store.clear()?,
        }
        *guard = session;
        (self.events)(Event::AuthChanged(guard.clone()));
        Ok(())
    }
}
