//! Atualização automática pelo GitHub Releases.
//!
//! Cada plataforma tem seu arquivo na release ([`Target`]): no Windows o instalador
//! (o app é um `.exe` mais a `libmpv`, vários arquivos, então nada de trocar o binário),
//! no Android o APK. O fluxo é o mesmo: achar a release mais nova, baixar o arquivo,
//! conferir o SHA-256 contra o `SHA256SUMS` da mesma release e entregar ao sistema
//! (o instalador silencioso no Windows; o instalador de pacotes no Android, que pede
//! a confirmação da pessoa).

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
/// Teto do arquivo: recusa o que destoa muito do esperado.
const MAX_SIZE: u64 = 400 * 1024 * 1024;

/// Qual arquivo da release serve a esta instalação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// `Higanbana-Setup-<versão>.exe` (Inno Setup).
    Windows,
    /// `Higanbana-<versão>-android.apk` (arm64, assinado sempre com a mesma chave).
    Android,
}

impl Target {
    /// Nome do arquivo desta plataforma na release `version` (sem o `v`).
    pub fn asset_name(self, version: &str) -> String {
        match self {
            Target::Windows => format!("Higanbana-Setup-{version}.exe"),
            Target::Android => format!("Higanbana-{version}-android.apk"),
        }
    }
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("rede: {0}")]
    Network(#[from] reqwest::Error),
    #[error("disco: {0}")]
    Io(#[from] std::io::Error),
    #[error("release sem o arquivo desta plataforma ou o SHA256SUMS")]
    MissingAsset,
    #[error("origem inesperada do download: {0}")]
    BadOrigin(String),
    #[error("tamanho inesperado do arquivo")]
    BadSize,
    #[error("o SHA-256 do arquivo não confere")]
    BadDigest,
    #[error("atualização automática só existe no Windows")]
    Unsupported,
}

/// Release mais nova que a instalada.
#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub notes: String,
    asset_name: String,
    asset_url: String,
    asset_size: u64,
    sums_url: String,
}

impl Release {
    /// Nome do arquivo que [`download`] grava.
    pub fn file_name(&self) -> &str {
        &self.asset_name
    }

    /// Tamanho do arquivo, em bytes.
    pub fn size(&self) -> u64 {
        self.asset_size
    }
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

/// Consulta a última release estável para o Windows. `None` = já está na mais nova (ou ainda não há release).
pub async fn check(web: &Client) -> Result<Option<Release>, UpdateError> {
    check_for(web, Target::Windows).await
}

/// Consulta a última release estável que traz o arquivo de `target`.
pub async fn check_for(web: &Client, target: Target) -> Result<Option<Release>, UpdateError> {
    let resp = web
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let api: ApiRelease = resp.error_for_status()?.json().await?;
    Ok(pick(api, version::current(), target))
}

fn pick(api: ApiRelease, current: &str, target: Target) -> Option<Release> {
    if api.draft || api.prerelease {
        return None;
    }
    let (new, cur) = (version::parse(&api.tag_name)?, version::parse(current)?);
    // Sem downgrade: só avança.
    if new <= cur {
        return None;
    }
    let version = api.tag_name.trim_start_matches('v').to_owned();
    let asset_name = target.asset_name(&version);
    let asset = api.assets.iter().find(|a| a.name == asset_name)?;
    let sums = api.assets.iter().find(|a| a.name == SUMS_ASSET)?;
    Some(Release {
        version,
        notes: api.body.unwrap_or_default(),
        asset_name,
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        sums_url: sums.browser_download_url.clone(),
    })
}

/// Baixa o arquivo da release para `dir`, confere tamanho e SHA-256 e devolve o caminho.
/// Em qualquer falha o arquivo parcial é apagado: nada fica pronto para rodar.
pub async fn download(web: &Client, release: &Release, dir: &Path) -> Result<PathBuf, UpdateError> {
    download_with_progress(web, release, dir, |_| {}).await
}

/// Como [`download`], avisando `progress` com os bytes recebidos a cada pedaço.
///
/// `dir` é só das atualizações: o que sobrou de versões anteriores é apagado. Se o
/// arquivo desta versão já está lá e confere, não baixa de novo (a pessoa fechou o
/// instalador e tocou em atualizar outra vez).
pub async fn download_with_progress(
    web: &Client,
    release: &Release,
    dir: &Path,
    progress: impl Fn(u64) + Send,
) -> Result<PathBuf, UpdateError> {
    for url in [&release.asset_url, &release.sums_url] {
        if !url.starts_with(DOWNLOAD_PREFIX) {
            return Err(UpdateError::BadOrigin(url.clone()));
        }
    }
    if release.asset_size == 0 || release.asset_size > MAX_SIZE {
        return Err(UpdateError::BadSize);
    }
    let sums = web.get(&release.sums_url).send().await?.error_for_status()?.text().await?;
    let expected = digest_for(&sums, &release.asset_name).ok_or(UpdateError::MissingAsset)?;

    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(&release.asset_name);
    remove_others(dir, &release.asset_name).await;
    if already_there(&path, release.asset_size, &expected).await {
        progress(release.asset_size);
        return Ok(path);
    }
    let result = fetch(web, &release.asset_url, &path, release.asset_size, &expected, &progress).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&path).await;
    }
    result.map(|_| path)
}

/// Apaga de `dir` tudo que não é `keep` (downloads de versões anteriores).
async fn remove_others(dir: &Path, keep: &str) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else { return };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_name() != keep {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

/// O arquivo já baixado tem o tamanho e o SHA-256 esperados.
async fn already_there(path: &Path, size: u64, expected: &str) -> bool {
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.len() == size => {}
        _ => return false,
    }
    let path = path.to_owned();
    let digest = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await;
    matches!(digest, Ok(Ok(d)) if d == expected)
}

async fn fetch(
    web: &Client,
    url: &str,
    path: &Path,
    size: u64,
    expected: &str,
    progress: &(impl Fn(u64) + Send),
) -> Result<(), UpdateError> {
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
        progress(total);
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
        let v = tag.trim_start_matches('v');
        ApiRelease {
            tag_name: tag.into(),
            body: None,
            draft: false,
            prerelease: false,
            assets: vec![
                asset(&Target::Windows.asset_name(v)),
                asset(&Target::Android.asset_name(v)),
                asset(SUMS_ASSET),
            ],
        }
    }

    #[test]
    fn only_newer_release_is_offered() {
        assert!(pick(api("v0.1.1"), "0.1.0", Target::Windows).is_some());
        assert!(pick(api("v0.1.0"), "0.1.0", Target::Windows).is_none());
        assert!(pick(api("v0.0.9"), "0.1.0", Target::Windows).is_none());
    }

    #[test]
    fn prerelease_and_missing_assets_are_skipped() {
        let mut pre = api("v0.2.0");
        pre.prerelease = true;
        assert!(pick(pre, "0.1.0", Target::Windows).is_none());
        let mut bare = api("v0.2.0");
        bare.assets.clear();
        assert!(pick(bare, "0.1.0", Target::Windows).is_none());
    }

    #[test]
    fn each_target_takes_its_own_file() {
        let android = pick(api("v0.2.0"), "0.1.0", Target::Android).unwrap();
        assert_eq!(android.file_name(), "Higanbana-0.2.0-android.apk");
        let windows = pick(api("v0.2.0"), "0.1.0", Target::Windows).unwrap();
        assert_eq!(windows.file_name(), "Higanbana-Setup-0.2.0.exe");

        // Release só com o Windows: o Android segue na versão que tem.
        let mut desktop_only = api("v0.2.0");
        desktop_only.assets.retain(|a| !a.name.ends_with(".apk"));
        assert!(pick(desktop_only, "0.1.0", Target::Android).is_none());
    }

    #[tokio::test]
    async fn reuses_a_verified_download_and_drops_old_ones() {
        let dir = std::env::temp_dir().join(format!("higanbana-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let body = b"apk de mentira";
        let path = dir.join("Higanbana-0.2.0-android.apk");
        std::fs::write(&path, body).unwrap();
        std::fs::write(dir.join("Higanbana-0.1.9-android.apk"), b"velho").unwrap();
        let digest = format!("{:x}", Sha256::digest(body));

        assert!(already_there(&path, body.len() as u64, &digest).await);
        assert!(!already_there(&path, body.len() as u64, &"0".repeat(64)).await);
        assert!(!already_there(&path, 3, &digest).await);

        remove_others(&dir, "Higanbana-0.2.0-android.apk").await;
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, vec![std::ffi::OsString::from("Higanbana-0.2.0-android.apk")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_digest_in_sha256sum_format() {
        let h = "a".repeat(64);
        let sums = format!("{h}  Higanbana-Setup-0.1.1.exe\n{}  outro.zip\n", "b".repeat(64));
        assert_eq!(digest_for(&sums, "Higanbana-Setup-0.1.1.exe"), Some(h));
        assert_eq!(digest_for(&sums, "falta.exe"), None);
    }
}
