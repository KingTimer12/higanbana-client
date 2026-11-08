//! Estado do player em `<app_data_dir>/player.json`: progresso por arquivo,
//! faixas lembradas por release e configurações.

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub position: f64,
    pub duration: f64,
    pub watched: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackPref {
    pub sub_lang: Option<String>,
    pub sub_title: Option<String>,
    pub sub_off: bool,
    pub audio_lang: Option<String>,
    pub audio_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub auto_next: bool,
    pub countdown_sec: u8,
    pub show_skip_intro: bool,
    pub sub_lang: String,
    pub audio_lang: String,
    pub cache_mib: u32,
    /// Miniatura do quadro ao passar o mouse na barra de posição.
    pub thumbnails: bool,
    /// Núcleos que o decodificador de vídeo pode usar; 0 deixa o mpv escolher.
    pub decode_threads: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_next: true,
            countdown_sec: 5,
            show_skip_intro: true,
            sub_lang: "por".into(),
            audio_lang: "jpn".into(),
            cache_mib: 150,
            thumbnails: true,
            decode_threads: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerData {
    pub progress: HashMap<String, Progress>,
    pub tracks: HashMap<String, TrackPref>,
    pub settings: Settings,
}

pub struct PlayerStore {
    path: PathBuf,
    data: Mutex<PlayerData>,
}

impl PlayerStore {
    /// Arquivo ausente ou corrompido → padrão.
    pub fn open(path: PathBuf) -> Self {
        let data = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, data: Mutex::new(data) }
    }

    pub fn data(&self) -> PlayerData {
        self.data.lock().unwrap().clone()
    }

    /// Aplica a mudança e grava (escreve em `.tmp` e renomeia).
    pub fn update(&self, f: impl FnOnce(&mut PlayerData)) {
        let bytes = {
            let mut data = self.data.lock().unwrap();
            f(&mut data);
            serde_json::to_vec(&*data)
        };
        let Ok(bytes) = bytes else { return };
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("higanbana-player-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("player.json")
    }

    #[test]
    fn defaults() {
        let s = PlayerData::default().settings;
        assert!(s.auto_next);
        assert_eq!(s.countdown_sec, 5);
        assert!(s.show_skip_intro);
        assert_eq!(s.sub_lang, "por");
        assert_eq!(s.audio_lang, "jpn");
        assert_eq!(s.cache_mib, 150);
        assert!(s.thumbnails);
        assert_eq!(s.decode_threads, 0);
    }

    #[test]
    fn old_settings_file_without_new_fields_keeps_defaults() {
        // Arquivo gravado antes de existirem `thumbnails` e `decodeThreads`.
        let old = r#"{"settings":{"autoNext":false,"cacheMib":300}}"#;
        let data: PlayerData = serde_json::from_str(old).unwrap();
        assert!(!data.settings.auto_next);
        assert_eq!(data.settings.cache_mib, 300);
        assert!(data.settings.thumbnails);
        assert_eq!(data.settings.decode_threads, 0);
    }

    #[test]
    fn roundtrip() {
        let path = tmp("roundtrip");
        let store = PlayerStore::open(path.clone());
        store.update(|d| {
            d.progress.insert(
                "Animes/x/ep1.mkv".into(),
                Progress { position: 61.5, duration: 1420.0, watched: false, updated_at: 1 },
            );
            d.settings.countdown_sec = 10;
        });
        let again = PlayerStore::open(path);
        assert_eq!(again.data(), store.data());
        assert_eq!(again.data().progress["Animes/x/ep1.mkv"].position, 61.5);
    }

    #[test]
    fn corrupt_file_falls_back() {
        let path = tmp("corrupt");
        std::fs::write(&path, "{lixo").unwrap();
        assert_eq!(PlayerStore::open(path).data(), PlayerData::default());
    }

    #[test]
    fn camel_case_json() {
        let json = serde_json::to_value(PlayerData::default()).unwrap();
        assert_eq!(json["settings"]["cacheMib"], 150);
        assert_eq!(json["settings"]["autoNext"], true);
    }
}
