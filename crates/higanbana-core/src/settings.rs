//! Preferências de exibição guardadas em `settings.json`: fonte das notas e
//! layout da lista de episódios.

use std::{path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::ratings::RatingProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EpisodeLayout {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct Data {
    #[serde(default)]
    rating_provider: RatingProvider,
    #[serde(default)]
    episode_layout: EpisodeLayout,
}

pub struct Settings {
    file: PathBuf,
    data: Mutex<Data>,
}

impl Settings {
    pub fn open(file: PathBuf) -> Self {
        let data = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { file, data: Mutex::new(data) }
    }

    pub fn rating_provider(&self) -> RatingProvider {
        self.data.lock().unwrap().rating_provider
    }

    pub fn set_rating_provider(&self, provider: RatingProvider) {
        self.update(|d| d.rating_provider = provider);
    }

    pub fn episode_layout(&self) -> EpisodeLayout {
        self.data.lock().unwrap().episode_layout
    }

    pub fn set_episode_layout(&self, layout: EpisodeLayout) {
        self.update(|d| d.episode_layout = layout);
    }

    fn update(&self, change: impl FnOnce(&mut Data)) {
        let mut data = self.data.lock().unwrap();
        change(&mut data);
        if let Err(e) = self.save(&data) {
            eprintln!("configurações: não salvou {}: {e}", self.file.display());
        }
    }

    /// Grava num arquivo temporário e renomeia, para nunca deixar JSON pela metade.
    fn save(&self, data: &Data) -> std::io::Result<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(data)?)?;
        std::fs::rename(&tmp, &self.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let dir = std::env::temp_dir().join(format!("settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("settings.json");
        let settings = Settings::open(file.clone());
        assert_eq!(settings.episode_layout(), EpisodeLayout::Grid);
        assert_eq!(settings.rating_provider(), RatingProvider::AniList);
        settings.set_episode_layout(EpisodeLayout::List);
        settings.set_rating_provider(RatingProvider::Tmdb);
        let again = Settings::open(file);
        assert_eq!(again.episode_layout(), EpisodeLayout::List);
        assert_eq!(again.rating_provider(), RatingProvider::Tmdb);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
