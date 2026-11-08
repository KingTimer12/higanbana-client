//! O que a interface pede ao player. Ela só manda caminho e tamanho;
//! quem fala com a nuvem é o núcleo.

use super::{
    Player,
    aniskip::{self, SkipTimes},
    store::{PlayerData, Progress, Settings, TrackPref},
};

impl Player {
    /// URL que o mpv deve abrir: o arquivo já baixado (mesmo tamanho) ou o proxy de stream.
    pub fn stream_url(&self, path: &str, size: u64) -> String {
        if let Some(local) = self.cloud.default_download_path(path)
            && std::fs::metadata(&local).is_ok_and(|m| m.is_file() && m.len() == size)
        {
            return local.to_string_lossy().into_owned();
        }
        self.proxy.url_for(path)
    }

    /// Como [`Player::stream_url`], para o gerador de miniaturas: o proxy dá a
    /// ele conexões próprias, sem disputar vaga com o vídeo que está tocando.
    pub fn preview_url(&self, path: &str, size: u64) -> String {
        if let Some(local) = self.cloud.default_download_path(path)
            && std::fs::metadata(&local).is_ok_and(|m| m.is_file() && m.len() == size)
        {
            return local.to_string_lossy().into_owned();
        }
        self.proxy.url_for_preview(path)
    }

    pub fn data(&self) -> PlayerData {
        self.store.data()
    }

    pub fn save_progress(&self, path: String, progress: Progress) {
        self.store.update(|d| {
            d.progress.insert(path, progress);
        });
    }

    /// Faixas escolhidas, lembradas por pasta de release.
    pub fn save_tracks(&self, release: String, pref: TrackPref) {
        self.store.update(|d| {
            d.tracks.insert(release, pref);
        });
    }

    pub fn save_settings(&self, settings: Settings) {
        self.store.update(|d| d.settings = settings);
    }

    /// Tempos de abertura/encerramento no AniSkip, com cache (inclusive "não encontrado").
    /// Falha de rede devolve vazio e não é guardada.
    pub async fn aniskip(&self, mal_id: u32, episode: u32, duration: f64) -> SkipTimes {
        let key = format!("{mal_id}:{episode}");
        if let Some(hit) = self.aniskip.get(&key) {
            return hit;
        }
        let Ok(resp) = self.http.get(aniskip::url(mal_id, episode, duration)).send().await else {
            return SkipTimes::default();
        };
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        match aniskip::parse_response(status, &body) {
            Some(times) => {
                self.aniskip.insert(key, times.clone());
                self.aniskip.flush();
                times
            }
            None => SkipTimes::default(),
        }
    }
}
