//! Atualização automática: confere o GitHub Releases ao abrir, avisa na barra de
//! título e, a pedido da pessoa, baixa o instalador, confere o SHA-256 e reinicia.

use higanbana_core::update::{self, Release};

use super::{AppState, Tone};

#[derive(Clone, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Available(Release),
    Downloading(Release),
}

impl AppState {
    /// Uma consulta por abertura. Falha de rede é silenciosa: o app segue funcionando.
    pub(super) fn check_update(&mut self) {
        if !cfg!(windows) || !matches!(self.update, UpdateState::Idle) {
            return;
        }
        let core = self.core.clone();
        self.run(async move { update::check(core.web()).await.ok().flatten() }, |s, release, _| {
            if let (Some(r), UpdateState::Idle) = (release, &s.update) {
                s.update = UpdateState::Available(r);
            }
        });
    }

    /// Baixa, confere e roda o instalador; ele fecha o app e abre o novo.
    pub fn install_update(&mut self) {
        let UpdateState::Available(release) = self.update.clone() else { return };
        self.update = UpdateState::Downloading(release.clone());
        let core = self.core.clone();
        let dir = self.core.paths().cache.join("update");
        let fut = async move {
            let path = update::download(core.web(), &release, &dir).await.map_err(|e| e.to_string())?;
            update::launch_installer(&path).map_err(|e| e.to_string())
        };
        self.run(fut, |s, result, cx| match result {
            Ok(()) => cx.quit(),
            Err(err) => {
                if let UpdateState::Downloading(r) = s.update.clone() {
                    s.update = UpdateState::Available(r);
                }
                s.toast(format!("Não foi possível atualizar: {err}"), Tone::Error);
            }
        });
    }
}
