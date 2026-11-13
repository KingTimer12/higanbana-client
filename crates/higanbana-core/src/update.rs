//! Atualização automática pelo GitHub Releases.
//!
//! O app é um `.exe` mais a `libmpv`, ou seja, vários arquivos: o contrato da doc do
//! GPUI Kit pede o instalador, não troca de binário. Então o fluxo é: achar a release
//! mais nova, baixar o instalador, conferir o SHA-256 contra o `SHA256SUMS` da mesma
//! release e rodar o instalador em modo silencioso (ele fecha o app e abre de novo).

use std::path::{Path, PathBuf};

use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use crate::version;

const REPO: &str = "KingTimer12/higanbana-client";
/// Tudo que se baixa precisa vir daqui (a API só aponta; a origem é conferida).
const DOWNLOAD_PREFIX: &str = "https://github.com/KingTimer12/higanbana-client/releases/download/";
const SUMS_ASSET: &str = "SHA256SUMS";
/// Teto do instalador: recusa o que destoa muito do esperado.
const MAX_SIZE: u64 = 400 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("rede: {0}")]
    Network(#[from] reqwest::Error),
    #[error("disco: {0}")]
    Io(#[from] std::io::Error),
    #[error("release sem o instalador ou o SHA256SUMS")]
    MissingAsset,
    #[error("origem inesperada do download: {0}")]
    BadOrigin(String),
    #[error("tamanho inesperado do instalador")]
    BadSize,
    #[error("o SHA-256 do instalador não confere")]
    BadDigest,
    #[error("atualização automática só existe no Windows")]
    Unsupported,
}

/// Release mais nova que a instalada.
#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub notes: String,
    setup_name: String,
    setup_url: String,
    setup_size: u64,
    sums_url: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Consulta a última release estável. `None` = já está na mais nova (ou ainda não há release).
pub async fn check(web: &Client) -> Result<Option<Release>, UpdateError> {
    let resp = web
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let api: ApiRelease = resp.error_for_status()?.json().await?;
    Ok(pick(api, version::current()))
}

fn pick(api: ApiRelease, current: &str) -> Option<Release> {
    if api.draft || api.prerelease {
        return None;
    }
    let (new, cur) = (version::parse(&api.tag_name)?, version::parse(current)?);
    // Sem downgrade: só avança.
    if new <= cur {
        return None;
    }
    let version = api.tag_name.trim_start_matches('v').to_owned();
    let setup_name = format!("Higanbana-Setup-{version}.exe");
    let setup = api.assets.iter().find(|a| a.name == setup_name)?;
    let sums = api.assets.iter().find(|a| a.name == SUMS_ASSET)?;
    Some(Release {
        version,
        notes: api.body.unwrap_or_default(),
        setup_name,
        setup_url: setup.browser_download_url.clone(),
        setup_size: setup.size,
        sums_url: sums.browser_download_url.clone(),
    })
}

/// Baixa o instalador para `dir`, confere tamanho e SHA-256 e devolve o caminho.
/// Em qualquer falha o arquivo parcial é apagado: nada fica pronto para rodar.
pub async fn download(web: &Client, release: &Release, dir: &Path) -> Result<PathBuf, UpdateError> {
    for url in [&release.setup_url, &release.sums_url] {
        if !url.starts_with(DOWNLOAD_PREFIX) {
            return Err(UpdateError::BadOrigin(url.clone()));
        }
    }
    if release.setup_size == 0 || release.setup_size > MAX_SIZE {
        return Err(UpdateError::BadSize);
    }
    let sums = web.get(&release.sums_url).send().await?.error_for_status()?.text().await?;
    let expected = digest_for(&sums, &release.setup_name).ok_or(UpdateError::MissingAsset)?;

    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(&release.setup_name);
    let result = fetch(web, &release.setup_url, &path, release.setup_size, &expected).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&path).await;
    }
    result.map(|_| path)
}

async fn fetch(web: &Client, url: &str, path: &Path, size: u64, expected: &str) -> Result<(), UpdateError> {
    let mut resp = web.get(url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(path).await?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    while let Some(chunk) = resp.chunk().await? {
        total += chunk.len() as u64;
        if total > size {
            return Err(UpdateError::BadSize);
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    if total != size {
        return Err(UpdateError::BadSize);
    }
    if format!("{:x}", hasher.finalize()) != expected {
        return Err(UpdateError::BadDigest);
    }
    Ok(())
}

/// Procura `<hash>  <arquivo>` (formato do `sha256sum`) e devolve o hash em minúsculas.
fn digest_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

/// Roda o instalador silencioso, solto do app. Quem chamou deve fechar o app em seguida:
/// o instalador abre o higanbana de novo quando termina.
pub fn launch_installer(setup: &Path) -> Result<(), UpdateError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        std::process::Command::new(setup)
            .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"])
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = setup;
        Err(UpdateError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(tag: &str) -> ApiRelease {
        let asset = |name: &str| ApiAsset { name: name.into(), size: 10, browser_download_url: format!("{DOWNLOAD_PREFIX}{tag}/{name}") };
        ApiRelease {
            tag_name: tag.into(),
            body: None,
            draft: false,
            prerelease: false,
            assets: vec![asset(&format!("Higanbana-Setup-{}.exe", tag.trim_start_matches('v'))), asset(SUMS_ASSET)],
        }
    }

    #[test]
    fn only_newer_release_is_offered() {
        assert!(pick(api("v0.1.1"), "0.1.0").is_some());
        assert!(pick(api("v0.1.0"), "0.1.0").is_none());
        assert!(pick(api("v0.0.9"), "0.1.0").is_none());
    }

    #[test]
    fn prerelease_and_missing_assets_are_skipped() {
        let mut pre = api("v0.2.0");
        pre.prerelease = true;
        assert!(pick(pre, "0.1.0").is_none());
        let mut bare = api("v0.2.0");
        bare.assets.clear();
        assert!(pick(bare, "0.1.0").is_none());
    }

    #[test]
    fn finds_digest_in_sha256sum_format() {
        let h = "a".repeat(64);
        let sums = format!("{h}  Higanbana-Setup-0.1.1.exe\n{}  outro.zip\n", "b".repeat(64));
        assert_eq!(digest_for(&sums, "Higanbana-Setup-0.1.1.exe"), Some(h));
        assert_eq!(digest_for(&sums, "falta.exe"), None);
    }
}
