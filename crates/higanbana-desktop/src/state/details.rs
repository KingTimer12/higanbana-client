//! Detalhe da obra (AniList) e episódios (TMDB) da tela de título. Ficam só
//! enquanto o título está aberto: navegar para fora solta tudo (RAM).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use higanbana_core::anilist::types::MediaDetail;
use higanbana_core::ratings::{self, RatingSource};
use higanbana_core::settings::EpisodeLayout;
use higanbana_core::tmdb::{matching::WorkQuery, WorkEpisodes};

use super::AppState;

#[derive(Default)]
pub(super) struct Details {
    /// `None` = pedindo; `Some(None)` = sem detalhe.
    media: HashMap<i64, Option<Option<Arc<MediaDetail>>>>,
    /// `None` = buscando; `Some(None)` = sem TMDB para a obra.
    episodes: HashMap<String, Option<Option<Arc<WorkEpisodes>>>>,
    episodes_failed: HashSet<String>,
}

/// Resultado do TMDB: falha passageira não fica guardada (`None`).
pub fn episodes_outcome<T, E>(r: Option<Result<Option<T>, E>>) -> Option<Option<Arc<T>>> {
    match r {
        Some(Ok(found)) => Some(found.map(Arc::new)),
        _ => None,
    }
}

impl AppState {
    /// Detalhe da obra `id`: `None` enquanto chega; pede na primeira vez.
    pub fn media_detail(&mut self, id: i64) -> Option<Option<Arc<MediaDetail>>> {
        if let Some(d) = self.details.media.get(&id) {
            return d.clone();
        }
        self.details.media.insert(id, None);
        let core = self.core.clone();
        self.run(async move { core.anilist().media(id).await }, move |s, r, cx| {
            let d = r.ok().and_then(|v| serde_json::from_value::<MediaDetail>(v).ok()).map(Arc::new);
            // Saiu do título enquanto vinha: descarta.
            if let Some(slot) = s.details.media.get_mut(&id) {
                *slot = Some(d);
                cx.notify();
            }
        });
        None
    }

    /// `None` = buscando ou não pedido; `Some(None)` = sem TMDB para a obra.
    pub fn episodes(&self, key: &str) -> Option<Option<Arc<WorkEpisodes>>> {
        if self.details.episodes_failed.contains(key) {
            return Some(None);
        }
        self.details.episodes.get(key).cloned().flatten()
    }

    /// Pede os episódios da obra uma vez por chave enquanto o título está aberto.
    pub fn want_episodes(&mut self, key: String, q: WorkQuery) {
        if self.details.episodes.contains_key(&key) {
            return;
        }
        self.details.episodes_failed.remove(&key);
        if !self.core.tmdb().configured() {
            self.details.episodes.insert(key, Some(None));
            return;
        }
        self.details.episodes.insert(key.clone(), None);
        let core = self.core.clone();
        self.run(async move { core.tmdb().work_episodes(&q).await }, move |s, r, cx| {
            if !s.details.episodes.contains_key(&key) {
                return;
            }
            match episodes_outcome(Some(r)) {
                Some(found) => {
                    s.details.episodes.insert(key, Some(found));
                }
                None => {
                    s.details.episodes.remove(&key);
                    s.details.episodes_failed.insert(key);
                }
            }
            cx.notify();
        });
    }

    /// Solta detalhes e episódios (saiu do título).
    pub(super) fn forget_details(&mut self) {
        self.details = Details::default();
    }

    /// Layout da aba Episódios (grade ou lista), guardado nas preferências.
    pub fn episode_layout(&self) -> EpisodeLayout {
        self.core.settings().episode_layout()
    }

    pub fn set_episode_layout(&mut self, layout: EpisodeLayout) {
        self.core.settings().set_episode_layout(layout);
    }

    /// Fonte das notas escolhida nas preferências.
    pub fn rating_source(&self) -> &'static dyn RatingSource {
        ratings::source(self.core.settings().rating_provider())
    }
}

/// Abre `url` no navegador padrão.
pub fn open_url(url: &str) {
    if let Err(e) = open::that_detached(url) {
        tracing::warn!(%e, url, "não foi possível abrir o navegador");
    }
}

#[cfg(test)]
mod tests {
    use super::episodes_outcome;

    #[test]
    fn falha_passageira_nao_fica() {
        assert_eq!(episodes_outcome::<u8, ()>(Some(Ok(Some(3)))).map(|o| o.map(|a| *a)), Some(Some(3)));
        assert_eq!(episodes_outcome::<u8, ()>(Some(Ok(None))).map(|o| o.map(|a| *a)), Some(None));
        assert!(episodes_outcome::<u8, ()>(Some(Err(()))).is_none());
        assert!(episodes_outcome::<u8, ()>(None).is_none());
    }
}
