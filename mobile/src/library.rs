//! Como uma obra se abre na nuvem: temporadas de uma franquia, versões
//! (pastas de release) e episódios com o progresso de quem assiste.

use higanbana_core::{
    Core,
    anilist::types::MediaSummary,
    tmdb::{matching::work_query, still_url},
    library::{
        base_name, folder_title, is_extra_folder, is_franchise, join_path, natural_cmp, parse_episode,
        parse_work_name, resolution_rank, version_label,
    },
    nuvem::FileEntry,
    player::logic::{episode_number, video_files},
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::on_core;

/// Tamanho do still pedido ao TMDB, o mesmo da grade do desktop.
const STILL_SIZE: &str = "w300";

/// Pastas de obra na nuvem.
#[module]
pub struct Library;

#[module]
impl Library {
    /// Abre a pasta de uma obra:
    /// `{ path, title, seasons, versions, defaultVersion, episodes }`.
    /// Franquia vem com `seasons` (cada uma se abre de novo por aqui); obra com
    /// releases vem com `versions`; arquivos soltos vêm em `episodes`.
    pub async fn open(path: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let listing = c.cloud().list(&path).await.map_err(|e| e.to_string())?;
            let title = folder_title(base_name(&path));
            let mut dirs: Vec<&FileEntry> = listing.files.iter().filter(|f| f.is_directory).collect();
            dirs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
            let names: Vec<&str> = dirs.iter().map(|d| d.name.as_str()).collect();

            if is_franchise(&names) {
                let seasons: Vec<JsonValue> = dirs
                    .iter()
                    .map(|d| {
                        let work = parse_work_name(&d.name);
                        json!({ "name": d.name, "path": join_path(&path, &d.name), "title": work.title, "year": work.year })
                    })
                    .collect();
                return Ok(json!({ "path": path, "title": title, "seasons": seasons, "versions": [], "defaultVersion": null, "episodes": [] }));
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
            }))
        })
        .await
    }

    /// Episódios (vídeos em ordem natural) de uma pasta, com o progresso salvo:
    /// `[{ name, path, size, number, label, quality, position, duration, watched }]`.
    pub async fn episodes(path: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let listing = c.cloud().list(&path).await.map_err(|e| e.to_string())?;
            Ok(JsonValue::Array(episodes_of(&c, &path, &listing.files)))
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
            let detail = serde_json::from_value::<MediaSummary>(media).map_err(|e| format!("detalhe do AniList: {e}"))?;
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

fn episodes_of(core: &Core, dir: &str, files: &[FileEntry]) -> Vec<JsonValue> {
    let progress = core.player().data().progress;
    video_files(files)
        .iter()
        .map(|f| {
            let path = join_path(dir, &f.name);
            let name = parse_episode(&f.name);
            let seen = progress.get(&path);
            json!({
                "name": f.name,
                "path": path,
                "size": f.size,
                "number": episode_number(&f.name),
                "label": name.label,
                "quality": name.quality,
                "position": seen.map_or(0.0, |p| p.position),
                "duration": seen.map_or(0.0, |p| p.duration),
                "watched": seen.is_some_and(|p| p.watched),
            })
        })
        .collect()
}
