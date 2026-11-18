//! Leitor de mangá (`Reader` em `higanbana/native`). O núcleo abre o `.cbz` da
//! nuvem por pedaços; cada página baixada vai para o cache do app e a tela a
//! mostra por `file://`, como as capas do fanart.
//!
//! Um arquivo pode ter vários capítulos dentro (coletâneas com `.cbz` internos
//! ou pastas): `units` os lista, e cada um tem as próprias páginas.

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

use higanbana_core::{
    library::base_name,
    nuvem::FileEntry,
    reader::{
        PageRef,
        logic::{chapter_label, expand_units, label_text, pages_of_group, sort_pages},
    },
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::{file_uri, on_core};

/// Pasta das páginas, dentro do cache do app.
const DIR: &str = "reader";
/// Páginas guardadas no cache; ao abrir um capítulo, as mais antigas saem.
const KEEP_PAGES: usize = 600;

/// Páginas de mangá e o progresso de leitura.
#[module]
pub struct Reader;

#[module]
impl Reader {
    /// Os capítulos de um arquivo: `[{ key, name, label, inner, group }]`. Um
    /// capítulo só (o arquivo inteiro, `key` = caminho) quando ele não se divide.
    pub async fn units(path: String, size: f64) -> Result<JsonValue, String> {
        let size = size as u64;
        on_core(move |c| async move {
            let dir = c.paths().cache.join(DIR);
            // Fora do caminho da leitura: o disco se arruma enquanto o índice baixa.
            std::thread::spawn(move || prune(&dir, KEEP_PAGES));
            let opened = c.reader().open(&path, size, None).await?;
            let file = FileEntry {
                name: base_name(&path).to_owned(),
                is_directory: false,
                size,
                modified: String::new(),
                extension: None,
            };
            let units = match expand_units(&path, &file, &opened) {
                Some(parts) => parts
                    .into_iter()
                    .map(|u| json!({ "key": u.key, "name": u.name, "label": label_text(&chapter_label(&u.name)), "inner": u.inner, "group": u.group }))
                    .collect(),
                None => vec![json!({
                    "key": path,
                    "name": file.name,
                    "label": label_text(&chapter_label(&file.name)),
                    "inner": null,
                    "group": null,
                })],
            };
            Ok(JsonValue::Array(units))
        })
        .await
    }

    /// As páginas de um capítulo, em ordem de leitura: as posições no índice do
    /// zip que [`Reader::page`] recebe.
    pub async fn pages(path: String, size: f64, inner: Option<String>, group: Option<String>) -> Result<Vec<f64>, String> {
        on_core(move |c| async move {
            let opened = c.reader().open(&path, size as u64, inner.as_deref()).await?;
            let pages = sort_pages(&pages_of_group(&opened.pages, group.as_deref()));
            Ok(pages.into_iter().map(|p| p.index as f64).collect())
        })
        .await
    }

    /// Uma página como `file://` do cache; baixa só se ainda não está lá.
    pub async fn page(path: String, size: f64, inner: Option<String>, index: f64) -> Result<String, String> {
        on_core(move |c| async move {
            let dir = c.paths().cache.join(DIR);
            let stem = page_stem(&path, inner.as_deref(), index as usize);
            if let Some(hit) = cached(&dir, &stem) {
                return Ok(file_uri(&hit));
            }
            let page = PageRef { size: size as u64, index: index as usize, inner, path };
            let (bytes, name) = c.reader().page(&page).await?;
            let file = dir.join(format!("{stem}.{}", extension(&name)));
            write_page(&dir, &file, &bytes).map_err(|e| format!("não foi possível guardar a página: {e}"))?;
            Ok(file_uri(&file))
        })
        .await
    }

    /// Guarda onde parou num capítulo (`key` de `units`). Conta como lido na última página.
    pub fn save_progress(key: String, page: f64, total: f64) -> Result<(), String> {
        crate::core()?.reader().store.set_progress(&key, page as usize, total as usize);
        Ok(())
    }

    /// Numa coletânea, guarda no arquivo em qual capítulo interno parou (o que a
    /// lista de capítulos mostra).
    pub fn save_summary(path: String, index: f64, count: f64, read: bool, unit: String, unit_name: String) -> Result<(), String> {
        crate::core()?.reader().store.set_summary(&path, index as usize, count as usize, read, &unit, &unit_name);
        Ok(())
    }

    /// Onde parou num capítulo: `{ page, total, read }`, ou `null`.
    pub fn progress(key: String) -> Result<JsonValue, String> {
        let data = crate::core()?.reader().store.data();
        Ok(data.progress.get(&key).map_or(JsonValue::Null, |p| json!({ "page": p.page, "total": p.total, "read": p.read })))
    }
}

/// Nome do arquivo da página no cache, sem a extensão: o mesmo para a mesma página.
fn page_stem(path: &str, inner: Option<&str>, index: usize) -> String {
    let mut h = DefaultHasher::new();
    (path, inner, index).hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Extensão da imagem pelo nome dentro do zip; sem uma conhecida, `img`.
fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "img".to_owned())
}

/// A página já guardada, com qualquer extensão.
fn cached(dir: &Path, stem: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_stem().is_some_and(|s| s == stem) && p.extension().is_some_and(|e| e != "tmp")
    })
}

/// Grava pelo lado (`.tmp`) e renomeia: a tela nunca lê uma página pela metade.
fn write_page(dir: &Path, file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, file)
}

/// Deixa só as `keep` páginas mais recentes.
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= keep {
        return;
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, path) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesma_pagina_mesmo_nome() {
        let a = page_stem("Mangás/x/Cap 01.cbz", None, 3);
        assert_eq!(a, page_stem("Mangás/x/Cap 01.cbz", None, 3));
        assert_ne!(a, page_stem("Mangás/x/Cap 01.cbz", None, 4));
        assert_ne!(a, page_stem("Mangás/x/Cap 01.cbz", Some("Cap 1.cbz"), 3));
    }

    #[test]
    fn extensao_da_imagem() {
        assert_eq!(extension("001/Página 01.JPG"), "jpg");
        assert_eq!(extension("sem-extensao"), "img");
        assert_eq!(extension("x.estranha!"), "img");
    }

    #[test]
    fn cache_guarda_e_poda() {
        let dir = std::env::temp_dir().join(format!("higanbana-reader-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..5 {
            write_page(&dir, &dir.join(format!("p{i}.jpg")), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(cached(&dir, "p4"), Some(dir.join("p4.jpg")));
        prune(&dir, 2);
        assert!(cached(&dir, "p0").is_none());
        assert!(cached(&dir, "p4").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
