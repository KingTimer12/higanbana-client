use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde::Serialize;
use reqwest::Method;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::{
    auth::AuthManager,
    events::{Event, EventFn},
    http::{AnitsuHttp, HttpResult},
};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub path: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub done: bool,
}

/// Nome de arquivo seguro para o sistema local.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches([' ', '.']);
    if trimmed.is_empty() { "download".into() } else { trimmed.into() }
}

/// Apaga o `.part` se o download não chegar ao fim: erro, ou a tarefa
/// cancelada (abortada no meio de um `await`).
struct PartFile(Option<PathBuf>);

impl Drop for PartFile {
    fn drop(&mut self) {
        if let Some(part) = self.0.take() {
            let _ = std::fs::remove_file(part);
        }
    }
}

/// Baixa `remote_path` em stream para `dest` (via `<dest>.part`), emitindo progresso.
pub async fn download(
    events: &EventFn,
    http: &AnitsuHttp,
    auth: &AuthManager,
    id: &str,
    remote_path: &str,
    dest: &Path,
) -> HttpResult<PathBuf> {
    let mut resp = http
        .send(auth, Method::GET, "download", |r| r.query(&[("path", remote_path)]))
        .await?;
    let total = resp.content_length();

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut guard = PartFile(Some(part.clone()));

    let result: HttpResult<u64> = async {
        let mut file = File::create(&part).await?;
        let mut downloaded = 0u64;
        let mut last_emit = Instant::now();
        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                last_emit = Instant::now();
                events(Event::DownloadProgress(DownloadProgress {
                    id: id.to_owned(),
                    path: remote_path.to_owned(),
                    downloaded,
                    total,
                    done: false,
                }));
            }
        }
        file.flush().await?;
        Ok(downloaded)
    }
    .await;

    let downloaded = result?;
    tokio::fs::rename(&part, dest).await?;
    guard.0 = None;
    events(Event::DownloadProgress(DownloadProgress {
        id: id.to_owned(),
        path: remote_path.to_owned(),
        downloaded,
        total,
        done: true,
    }));
    Ok(dest.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(
            sanitize_file_name("[Troidex] B-The Beginning - Ep. 01 [BD-Rip_1080p_H264_Dual].mkv"),
            "[Troidex] B-The Beginning - Ep. 01 [BD-Rip_1080p_H264_Dual].mkv"
        );
        assert_eq!(sanitize_file_name("a/b\\c:d?.mkv"), "a_b_c_d_.mkv");
        assert_eq!(sanitize_file_name(".."), "download");
    }

    #[test]
    fn part_some_quando_o_download_nao_termina() {
        let dir = std::env::temp_dir().join(format!("higanbana-part-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (aborted, finished) = (dir.join("a.mkv.part"), dir.join("b.mkv.part"));
        std::fs::write(&aborted, b"x").unwrap();
        std::fs::write(&finished, b"x").unwrap();
        drop(PartFile(Some(aborted.clone())));
        let mut done = PartFile(Some(finished.clone()));
        done.0 = None;
        drop(done);
        assert!(!aborted.exists());
        assert!(finished.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
