//! Atualização automática: confere o GitHub Releases ao abrir, avisa na barra de
//! título e, a pedido da pessoa, baixa o instalador, confere o SHA-256 e reinicia.

use gpui_kit::Context;
use higanbana_core::update::{self, Release};

use crate::state::{AppState, Tone};

#[derive(Clone, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Available(Release),
    Downloading(Release),
}

impl AppState {
    /// Uma consulta por abertura. Falha de rede é silenciosa: o app segue funcionando.
    pub fn check_update(&mut self, cx: &mut Context<Self>) {
        if !cfg!(windows) || !matches!(self.update, UpdateState::Idle) {
            return;
        }
        let core = self.core.clone();
        let task = self.core.spawn(async move { update::check(core.web()).await });
        cx.spawn(async move |this, cx| {
            if let Some(release) = task.await.ok().and_then(Result::ok).flatten() {
                let _ = this.update(cx, |s, cx| {
                    if matches!(s.update, UpdateState::Idle) {
                        s.update = UpdateState::Available(release);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// Baixa, confere e roda o instalador; ele fecha o app e abre o novo.
    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(release) = self.update.clone() else { return };
        self.update = UpdateState::Downloading(release.clone());
        cx.notify();
        let core = self.core.clone();
        let dir = self.core.paths().cache.join("update");
        let task = self.core.spawn(async move {
            let path = update::download(core.web(), &release, &dir).await?;
            update::launch_installer(&path)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |s, cx| match result {
                Ok(Ok(())) => cx.quit(),
                other => {
                    let err = match other {
                        Ok(Err(e)) => e.to_string(),
                        Err(e) => e.to_string(),
                        Ok(Ok(())) => unreachable!(),
                    };
                    if let UpdateState::Downloading(r) = s.update.clone() {
                        s.update = UpdateState::Available(r);
                    }
                    s.toast(format!("Não foi possível atualizar: {err}"), Tone::Error, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
