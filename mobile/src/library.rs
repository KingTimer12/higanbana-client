//! Como uma obra se abre na nuvem: obras de uma franquia, versões (pastas de
//! release), episódios com o progresso de quem assiste e capítulos de mangá
//! com o de quem lê.

use higanbana_core::{
    Core,
    anilist::types::MediaDetail,
    franchise::{franchise_span, franchise_tabs},
    reader::logic::{chapter_label, comic_files, label_text},
    tmdb::{matching::work_query, still_url},
    library::{
        base_name, folder_title, is_extra_folder, parent_path, is_franchise, join_path, natural_cmp, parse_episode,
        parse_work_name, resolution_rank, version_label,
    },
    nuvem::FileEntry,
    offline::OfflineStatus,
    player::logic::{episode_number, video_files},
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::on_core;

/// Os arquivos de uma pasta da nuvem; sem rede, os baixados dela (o player e o
/// leitor seguem para o próximo baixado). Sem nenhum baixado, o erro da nuvem.
async fn listing_or_offline(c: &Core, path: &str) -> Result<Vec<FileEntry>, String> {
    match c.cloud().list(path).await {
        Ok(listing) => Ok(listing.files),
        Err(e) => {
            let saved: Vec<FileEntry> = c
                .offline()
                .list()
                .into_iter()
                .filter(|i| i.status == OfflineStatus::Done && parent_path(&i.path) == path)
                .map(|i| FileEntry { name: i.name, is_directory: false, size: i.size, modified: String::new(), extension: None })
                .collect();
            if saved.is_empty() { Err(e.to_string()) } else { Ok(saved) }
        }
    }
}

/// Tamanho do still pedido ao TMDB, o mesmo da grade do desktop.
const STILL_SIZE: &str = "w300";

/// Pastas de obra na nuvem.
#[module]
pub struct Library;

#[module]
impl Library {
    /// Abre a pasta de uma obra:
    /// `{ path, title, seasons, span, versions, defaultVersion, episodes, chapters }`.
    /// Franquia vem com `seasons` (cada obra se abre de novo por aqui) e `span`
    /// (`Franquia · 2004–2019`); obra com releases vem com `versions`; arquivos
    /// soltos vêm em `episodes` (vídeos) e `chapters` (mangá).
    pub async fn open(path: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let listing = c.cloud().list(&path).await.map_err(|e| e.to_string())?;
            let title = folder_title(base_name(&path));
            let mut dirs: Vec<&FileEntry> = listing.files.iter().filter(|f| f.is_directory).collect();
            dirs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
            let names: Vec<&str> = dirs.iter().map(|d| d.name.as_str()).collect();

            if is_franchise(&names) {
                let works: Vec<_> = names.iter().map(|n| parse_work_name(n)).collect();
                let span = franchise_span(&works.iter().collect::<Vec<_>>());
                let seasons: Vec<JsonValue> = dirs
                    .iter()
                    .zip(&works)
                    .map(|(d, work)| json!({ "name": d.name, "path": join_path(&path, &d.name), "title": work.title, "year": work.year }))
                    .collect();
                return Ok(json!({
                    "path": path,
                    "title": title,
                    "seasons": seasons,
                    "span": span,
                    "versions": [],
                    "defaultVersion": null,
                    "episodes": [],
                    "chapters": [],
                }));
            }

            let versions: Vec<&FileEntry> = dirs.into_iter().filter(|d| !is_extra_folder(&d.name)).collect();
            let remembered = c.prefs().version(&path);
            let default = default_version(&versions, remembered.as_deref(), &title);
            let versions: Vec<JsonValue> = versions
                .iter()
                .map(|v| {
                    let label = version_label(&v.name, &title);
                    json!({
                        "folder": v.name,
                        "path": join_path(&path, &v.name),
                        "name": label.name,
                        "resolution": label.resolution,
                        "source": label.source,
                        "audio": label.audio,
                    })
                })
                .collect();
            Ok(json!({
                "path": path,
                "title": title,
                "seasons": [],
                "versions": versions,
                "defaultVersion": default,
                "episodes": episodes_of(&c, &path, &listing.files),
                "chapters": chapters_of(&c, &path, &listing.files),
            }))
        })
        .await
    }

    /// Capítulos de mangá (`.cbz`, `.zip`, `.cbr`, `.rar` em ordem natural) de uma
    /// pasta, com o progresso de leitura:
    /// `[{ name, path, size, label, page, total, read, unitName }]`.
    pub async fn chapters(path: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let files = listing_or_offline(&c, &path).await?;
            Ok(JsonValue::Array(chapters_of(&c, &path, &files)))
        })
        .await
    }

    /// As abas de uma franquia, como no desktop: cada obra pelo nome da pasta e
    /// pelo formato do AniList (`null` enquanto não se sabe), na ordem de `names`.
    /// `[{ id, label, works: [índice] }]`, só as abas com obras.
    pub fn franchise_tabs(names: Vec<String>, formats: Vec<Option<String>>) -> JsonValue {
        let works: Vec<_> = names.iter().map(|n| parse_work_name(n)).collect();
        let input: Vec<_> = works.iter().enumerate().map(|(i, w)| (w, formats.get(i).and_then(|f| f.as_deref()))).collect();
        franchise_tabs(&input)
            .into_iter()
            .map(|(tab, ix)| json!({ "id": tab.id(), "label": tab.label(ix.len()), "works": ix }))
            .collect()
    }

    /// Episódios (vídeos em ordem natural) de uma pasta, com o progresso salvo:
    /// `[{ name, path, size, number, label, quality, position, duration, watched }]`.
    pub async fn episodes(path: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let files = listing_or_offline(&c, &path).await?;
            Ok(JsonValue::Array(episodes_of(&c, &path, &files)))
        })
        .await
    }

    /// Título, sinopse e miniatura do TMDB de cada arquivo, na ordem de `files`
    /// (`null` onde não casa), como a aba Episódios do desktop: a obra se acha
    /// pelo detalhe do AniList (`media_id`, lido aqui, do cache) e pelo nome da
    /// pasta (`folder`). Sem TMDB no build, filme ou obra não achada: todos `null`.
    /// `[{ title, overview, still, airDate, runtime, vote } | null]`.
    ///
    /// Recebe o id, não o detalhe: um JSON que passa pelo TS volta com os
    /// inteiros como `f64` (`154587.0`), e o `MediaSummary` não os aceita.
    pub async fn tmdb_episodes(media_id: f64, folder: String, files: Vec<String>) -> Result<JsonValue, String> {
        let n = files.len();
        let none = move || JsonValue::Array(vec![JsonValue::Null; n]);
        on_core(move |c| async move {
            if !c.tmdb().configured() {
                return Ok(none());
            }
            let media = c.anilist().media(media_id as i64).await.map_err(|e| e.to_string())?;
            let detail = serde_json::from_value::<MediaDetail>(media).map_err(|e| format!("detalhe do AniList: {e}"))?;
            let Some(query) = work_query(&detail, &folder) else { return Ok(none()) };
            let Some(work) = c.tmdb().work_episodes(&query).await.map_err(|e| e.to_string())? else {
                return Ok(none());
            };
            let names: Vec<&str> = files.iter().map(String::as_str).collect();
            Ok(JsonValue::Array(
                work.links(&names)
                    .into_iter()
                    .map(|ep| match ep {
                        Some(e) => json!({
                            "title": e.title,
                            "overview": e.overview,
                            "still": e.still_path.as_deref().map(|p| still_url(p, STILL_SIZE)),
                            "airDate": e.air_date,
                            "runtime": e.runtime,
                            "vote": (e.vote_count > 0).then_some(e.vote_average),
                        }),
                        None => JsonValue::Null,
                    })
                    .collect(),
            ))
        })
        .await
    }

    /// Lembra a versão (pasta de release) escolhida para uma obra.
    pub fn remember_version(work_path: String, folder: String) -> Result<(), String> {
        crate::core()?.prefs().set_version(&work_path, &folder);
        Ok(())
    }
}

/// A versão lembrada, se ainda existe; senão a de maior resolução (a primeira, no empate).
fn default_version(versions: &[&FileEntry], remembered: Option<&str>, work_title: &str) -> Option<String> {
    if let Some(r) = remembered.filter(|r| versions.iter().any(|v| v.name == *r)) {
        return Some(r.to_owned());
    }
    let rank = |v: &FileEntry| resolution_rank(&version_label(&v.name, work_title));
    versions
        .iter()
        .fold(None::<&FileEntry>, |best, v| match best {
            Some(b) if rank(b) >= rank(v) => Some(b),
            _ => Some(v),
        })
        .map(|v| v.name.clone())
}

fn chapters_of(core: &Core, dir: &str, files: &[FileEntry]) -> Vec<JsonValue> {
    let comics = comic_files(files);
    if comics.is_empty() {
        return Vec::new();
    }
    let reading = core.reader().store.data().progress;
    comics
        .iter()
        .map(|f| {
            let path = join_path(dir, &f.name);
            let seen = reading.get(&path);
            json!({
                "name": f.name,
                "path": path,
                "size": f.size,
                "label": label_text(&chapter_label(&f.name)),
                "page": seen.map_or(0, |p| p.page),
                "total": seen.map_or(0, |p| p.total),
                "read": seen.is_some_and(|p| p.read),
                "unitName": seen.and_then(|p| p.unit_name.clone()),
            })
        })
        .collect()
}

fn episodes_of(core: &Core, dir: &str, files: &[FileEntry]) -> Vec<JsonValue> {
    let videos = video_files(files);
    let paths: Vec<String> = videos.iter().map(|f| join_path(dir, &f.name)).collect();
    // Só o progresso destes arquivos, sem clonar o mapa inteiro.
    let seen: Vec<_> = core.player().with_data(|d| {
        paths.iter().map(|path| d.progress.get(path).map(|p| (p.position, p.duration, p.watched))).collect()
    });
    videos
        .iter()
        .zip(paths)
        .zip(seen)
        .map(|((f, path), seen)| {
            let name = parse_episode(&f.name);
            json!({
                "name": f.name,
                "path": path,
                "size": f.size,
                "number": episode_number(&f.name),
                "label": name.label,
                "quality": name.quality,
                "position": seen.map_or(0.0, |p| p.0),
                "duration": seen.map_or(0.0, |p| p.1),
                "watched": seen.is_some_and(|p| p.2),
            })
        })
        .collect()
}
