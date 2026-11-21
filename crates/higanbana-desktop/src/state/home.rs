//! Dados do início: listas de ranking conferidas no acervo (banner e Descobrir).
//! Cada lista chega item a item; sair do início solta tudo e cancela a
//! conferência em andamento (voltar recomeça, rápido pelo cache do núcleo).

use std::collections::HashMap;

use higanbana_core::anilist::{types::RankedMedia, MediaType, Ranking};

use super::AppState;

/// Obras confirmadas de uma lista e se a conferência terminou.
pub struct Slot {
    pub items: Vec<(RankedMedia, String)>,
    pub done: bool,
    job: Option<tokio::task::AbortHandle>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        if let Some(j) = self.job.take() {
            j.abort();
        }
    }
}

#[derive(Default)]
pub struct HomeData {
    slots: HashMap<(MediaType, Ranking), Slot>,
}

impl HomeData {
    pub fn slot(&self, media: MediaType, ranking: Ranking) -> Option<&Slot> {
        self.slots.get(&(media, ranking))
    }

    /// Sai de uma lista que não terminou: cancela (lista escondida não gasta orçamento).
    pub fn leave(&mut self, media: MediaType, ranking: Ranking) {
        if self.slots.get(&(media, ranking)).is_some_and(|s| !s.done) {
            self.slots.remove(&(media, ranking));
        }
    }

    fn push(&mut self, key: (MediaType, Ranking), item: (RankedMedia, String)) -> bool {
        self.slots.get_mut(&key).map(|s| s.items.push(item)).is_some()
    }

    fn finish(&mut self, key: (MediaType, Ranking)) {
        if let Some(s) = self.slots.get_mut(&key) {
            s.done = true;
            s.job = None;
        }
    }
}

impl AppState {
    /// Confere `ranking` de `media` no acervo (uma vez por lista enquanto no início).
    pub fn want_home_list(&mut self, media: MediaType, ranking: Ranking, n: usize) {
        let key = (media, ranking);
        if self.home.slots.contains_key(&key) {
            return;
        }
        let (core, post, budget) = (self.core.clone(), self.post(), self.core.home_budget());
        let item_post = post.clone();
        let job = self.core.spawn(async move {
            core.discover(media, ranking, n, budget, move |m, p| {
                item_post(Box::new(move |s: &mut AppState, cx| {
                    if s.home.push(key, (m, p)) {
                        cx.notify();
                    }
                }));
            })
            .await;
            post(Box::new(move |s: &mut AppState, cx| {
                s.home.finish(key);
                cx.notify();
            }));
        });
        self.home.slots.insert(key, Slot { items: Vec::new(), done: false, job: Some(job.abort_handle()) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(done: bool) -> Slot {
        Slot { items: Vec::new(), done, job: None }
    }

    #[test]
    fn sair_de_lista_inacabada_cancela() {
        let mut h = HomeData::default();
        h.slots.insert((MediaType::Anime, Ranking::Popular), slot(false));
        h.slots.insert((MediaType::Anime, Ranking::Trending), slot(true));
        h.leave(MediaType::Anime, Ranking::Popular);
        h.leave(MediaType::Anime, Ranking::Trending);
        assert!(h.slot(MediaType::Anime, Ranking::Popular).is_none());
        assert!(h.slot(MediaType::Anime, Ranking::Trending).is_some());
    }

    #[test]
    fn item_de_lista_solta_e_descartado() {
        let mut h = HomeData::default();
        assert!(!h.push((MediaType::Anime, Ranking::Popular), (serde_json::from_str(r#"{"id":1}"#).unwrap(), "x".into())));
    }
}
