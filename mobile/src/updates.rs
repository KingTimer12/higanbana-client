//! Versão do app e atualização pelo GitHub Releases (`Updates` em `higanbana/native`).
//!
//! O núcleo acha a release, baixa o APK e confere o SHA-256; quem instala é o
//! instalador de pacotes do Android, aberto pelo host (`UpdateInstaller.java`)
//! quando a tela chama `openUrl` com o endereço de [`Updates::download`].
//! Fora do Android não há o que instalar: `check` responde `null`.

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

use higanbana_core::{
    update::{self, Release, Target},
    version,
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::on_core;

/// A release achada pelo último `check`, que o `download` baixa.
static FOUND: Mutex<Option<Release>> = Mutex::new(None);
/// Bytes recebidos e total do download em andamento (a tela lê com `progress`).
static RECEIVED: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);

/// Pasta das atualizações, dentro do cache do app. O host Android só entrega ao
/// instalador o que está aqui (`res/xml/update_paths.xml`).
const DIR: &str = "update";

/// Versão instalada e atualização do app.
#[module]
pub struct Updates;

#[module]
impl Updates {
    /// A versão instalada (`0.1.0`), a mesma do arquivo `VERSION` do repositório.
    pub fn version() -> String {
        version::current().to_owned()
    }

    /// Procura uma versão mais nova: `{ version, notes, size }`, ou `null`.
    pub async fn check() -> Result<JsonValue, String> {
        if !cfg!(target_os = "android") {
            return Ok(JsonValue::Null);
        }
        let found = on_core(|c| async move { update::check_for(c.web(), Target::Android).await.map_err(|e| e.to_string()) }).await?;
        let reply = found
            .as_ref()
            .map_or(JsonValue::Null, |r| json!({ "version": r.version, "notes": r.notes, "size": r.size() }));
        *FOUND.lock().map_err(|e| e.to_string())? = found;
        Ok(reply)
    }

    /// Baixa e confere o APK achado por `check`; devolve o endereço que a tela
    /// passa a `openUrl` para abrir o instalador.
    pub async fn download() -> Result<String, String> {
        let release = FOUND.lock().map_err(|e| e.to_string())?.clone().ok_or("nenhuma atualização encontrada")?;
        RECEIVED.store(0, Ordering::Relaxed);
        TOTAL.store(release.size(), Ordering::Relaxed);
        let name = release.file_name().to_owned();
        on_core(move |c| async move {
            let dir = c.paths().cache.join(DIR);
            let progress = |received| RECEIVED.store(received, Ordering::Relaxed);
            update::download_with_progress(c.web(), &release, &dir, progress).await.map_err(|e| e.to_string())
        })
        .await?;
        Ok(install_url(&name))
    }

    /// Quanto do download chegou, de 0 a 1.
    pub fn progress() -> f64 {
        let total = TOTAL.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (RECEIVED.load(Ordering::Relaxed) as f64 / total as f64).min(1.0)
    }
}

/// O endereço que o host Android reconhece e troca pelo instalador de pacotes.
fn install_url(file: &str) -> String {
    format!("higanbana://update/install?file={file}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endereco_do_instalador() {
        assert_eq!(
            install_url("Higanbana-0.2.0-android.apk"),
            "higanbana://update/install?file=Higanbana-0.2.0-android.apk"
        );
    }

    #[test]
    fn versao_vem_do_arquivo_version() {
        assert!(version::parse(&Updates::version()).is_some());
    }
}
