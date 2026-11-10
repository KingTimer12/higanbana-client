//! O que o player pede ao núcleo: URL de stream, progresso, faixas lembradas
//! e tempos de abertura/encerramento. Quem toca é a interface (mpv).

use higanbana_core::{
    auth::session::now,
    library::{Place, base_name, classify, folder_title, parent_path},
    player::{
        aniskip::SkipTimes,
        logic::{
            Chapter, SubPick, Track, TrackKind, classify_chapters, episode_label, estimate_ending, is_watched,
            pick_tracks,
        },
        store::{Progress, TrackPref},
    },
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::{core, on_core};

/// Menor posição (s) para um episódio contar como "em andamento".
const RESUME_MIN_SEC: f64 = 30.0;
const CONTINUE_LIMIT: usize = 12;

/// Estado e regras do player de vídeo.
#[module]
pub struct Player;

#[module]
impl Player {
    /// URL que o player abre para um arquivo da nuvem (proxy local com a sessão).
    pub fn stream_url(path: String, size: f64) -> Result<String, String> {
        Ok(core()?.player().stream_url(&path, size as u64))
    }

    /// Preferências do player: `{ autoNext, countdownSec, showSkipIntro, subLang, audioLang }`.
    pub fn settings() -> Result<JsonValue, String> {
        serde_json::to_value(core()?.player().data().settings).map_err(|e| e.to_string())
    }

    /// Guarda a posição de um arquivo; resolve com `watched` (90% ou começo do encerramento).
    pub fn save_progress(path: String, position: f64, duration: f64, ending_start: Option<f64>) -> Result<bool, String> {
        if duration <= 0.0 {
            return Ok(false);
        }
        let ed = ending_start.map(|start| higanbana_core::player::aniskip::Interval { start, end: duration });
        let watched = is_watched(position, duration, ed);
        core()?.player().save_progress(path, Progress { position, duration, watched, updated_at: now() });
        Ok(watched)
    }

    /// Faixas a aplicar ao abrir um arquivo: `{ sub, audio }`, com `sub` um id,
    /// `"off"` ou `null` (deixar como o player escolheu). Vale a escolha lembrada
    /// do release (`dir`) e, sem ela, os idiomas preferidos.
    pub fn pick_tracks(tracks: JsonValue, dir: String) -> Result<JsonValue, String> {
        let tracks: Vec<Track> = serde_json::from_value(tracks).map_err(|e| e.to_string())?;
        let data = core()?.player().data();
        let (sub, audio) = pick_tracks(&tracks, data.tracks.get(&dir), &data.settings.sub_lang, &data.settings.audio_lang);
        let sub = match sub {
            Some(SubPick::Id(id)) => json!(id),
            Some(SubPick::Off) => json!("off"),
            None => JsonValue::Null,
        };
        Ok(json!({ "sub": sub, "audio": audio }))
    }

    /// Lembra as faixas escolhidas para o release (`dir`): `sub` e `audio` são
    /// ids de `tracks`; `sub` nulo desliga a legenda.
    pub fn remember_tracks(tracks: JsonValue, dir: String, sub: Option<f64>, audio: Option<f64>) -> Result<(), String> {
        let tracks: Vec<Track> = serde_json::from_value(tracks).map_err(|e| e.to_string())?;
        let find = |kind: TrackKind, id: Option<f64>| {
            let id = id? as i64;
            tracks.iter().find(|t| t.kind == kind && t.id == id)
        };
        let (s, a) = (find(TrackKind::Sub, sub), find(TrackKind::Audio, audio));
        let pref = TrackPref {
            sub_lang: s.and_then(|t| t.lang.clone()),
            sub_title: s.and_then(|t| t.title.clone()),
            sub_off: s.is_none(),
            audio_lang: a.and_then(|t| t.lang.clone()),
            audio_title: a.and_then(|t| t.title.clone()),
        };
        core()?.player().save_tracks(dir, pref);
        Ok(())
    }

    /// Abertura e encerramento do episódio: `{ op, ed }`, cada um `{ start, end }` ou `null`.
    /// Vêm dos capítulos do arquivo; sem eles, do AniSkip (precisa de `malId` e
    /// `episode`); sem nada, o encerramento é estimado em episódios de TV.
    pub async fn skip_times(chapters: JsonValue, duration: f64, mal_id: Option<f64>, episode: Option<f64>) -> Result<JsonValue, String> {
        let chapters: Vec<Chapter> = serde_json::from_value(chapters).unwrap_or_default();
        let mut times = classify_chapters(&chapters, duration);
        if times.op.is_none() && times.ed.is_none() {
            if let (Some(mal), Some(ep)) = (mal_id, episode) {
                times = on_core(move |c| async move { Ok(c.player().aniskip(mal as u32, ep as u32, duration).await) }).await?;
            }
        }
        if times.ed.is_none() {
            times.ed = estimate_ending(duration, true);
        }
        Ok(skip_json(&times))
    }

    /// Episódios em andamento, do mais recente:
    /// `[{ path, dir, name, title, label, position, duration }]`.
    pub fn continue_watching() -> Result<JsonValue, String> {
        let mut recent: Vec<(String, Progress)> = core()?
            .player()
            .data()
            .progress
            .into_iter()
            .filter(|(_, p)| !p.watched && p.position >= RESUME_MIN_SEC)
            .collect();
        recent.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
        Ok(recent
            .into_iter()
            .take(CONTINUE_LIMIT)
            .map(|(path, p)| {
                let name = base_name(&path).to_owned();
                let dir = parent_path(&path);
                let title = match classify(&path) {
                    Place::Title { title_path, .. } => folder_title(base_name(&title_path)),
                    _ => folder_title(base_name(&dir)),
                };
                json!({
                    "path": path,
                    "dir": dir,
                    "name": name,
                    "title": title,
                    "label": episode_label(&name),
                    "position": p.position,
                    "duration": p.duration,
                })
            })
            .collect())
    }

    /// Pausa a indexação do catálogo enquanto um vídeo toca (a nuvem limita as listagens).
    pub fn set_playing(playing: bool) -> Result<(), String> {
        core()?.anime_index().set_paused(playing);
        Ok(())
    }
}

fn skip_json(times: &SkipTimes) -> JsonValue {
    serde_json::to_value(times).unwrap_or(JsonValue::Null)
}
