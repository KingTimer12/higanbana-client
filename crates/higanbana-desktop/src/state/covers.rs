//! Capa de cada obra: fanart local (`file:<caminho>`) quando já baixado, senão a
//! URL do AniList. O fanart é pedido uma vez por id na sessão; falha volta ao AniList.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use higanbana_core::anilist::{types::MediaSummary, MediaType};

use super::images::file_key;
use super::AppState;

#[derive(Default)]
pub(super) struct Covers {
    /// `None` = pedindo; `Some(None)` = sem fanart; `Some(Some(p))` = arquivo local.
    map: HashMap<i64, Option<Option<PathBuf>>>,
    failed: HashSet<i64>,
}

impl Covers {
    /// Chave da capa já conhecida, ou `Err(())` se ainda falta pedir o fanart.
    fn key(&self, m: &MediaSummary, small: bool) -> Result<Option<String>, ()> {
        let anilist = if small { m.cover_small() } else { m.cover() }.map(str::to_owned);
        if m.media_type == Some(MediaType::Manga) || self.failed.contains(&m.id) {
            return Ok(anilist);
        }
        match self.map.get(&m.id) {
            None => Err(()),
            Some(Some(Some(p))) => Ok(Some(file_key(p))),
            Some(_) => Ok(anilist),
        }
    }

    /// Nova sessão de navegação: dá outra chance a quem falhou.
    pub(super) fn retry_failed(&mut self) {
        self.failed.clear();
    }
}

impl AppState {
    /// Chave de imagem da capa (pode ser chamada no render; pede o fanart uma vez).
    pub fn cover_key(&mut self, m: &MediaSummary, small: bool) -> Option<String> {
        if let Ok(k) = self.covers.key(m, small) {
            return k;
        }
        if let Some(p) = self.core.fanart().cover_cached(m) {
            self.covers.map.insert(m.id, Some(Some(p)));
            return self.covers.key(m, small).ok().flatten();
        }
        self.covers.map.insert(m.id, None);
        let (core, media, id) = (self.core.clone(), m.clone(), m.id);
        self.run(async move { core.fanart().cover(&media).await }, move |s, res, cx| {
            match res {
                Ok(path) => {
                    s.covers.map.insert(id, Some(path));
                }
                // Rede: AniList agora; tenta de novo na próxima navegação.
                Err(_) => {
                    s.covers.map.remove(&id);
                    s.covers.failed.insert(id);
                }
            }
            cx.notify();
        });
        self.covers.key(m, small).ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(id: i64, manga: bool) -> MediaSummary {
        let mut m: MediaSummary = serde_json::from_str(&format!(r#"{{"id":{id},"coverImage":{{"extraLarge":"https://a/{id}.jpg","large":"https://a/{id}s.jpg"}}}}"#)).unwrap();
        m.media_type = Some(if manga { MediaType::Manga } else { MediaType::Anime });
        m
    }

    #[test]
    fn manga_usa_anilist_direto() {
        let c = Covers::default();
        assert_eq!(c.key(&media(1, true), false), Ok(Some("https://a/1.jpg".into())));
    }

    #[test]
    fn anime_sem_pedido_pede_fanart() {
        assert_eq!(Covers::default().key(&media(2, false), false), Err(()));
    }

    #[test]
    fn fanart_local_vira_chave_de_arquivo() {
        let mut c = Covers::default();
        c.map.insert(3, Some(Some(PathBuf::from("capas").join("3.jpg"))));
        assert!(c.key(&media(3, false), false).unwrap().unwrap().starts_with("file:"));
    }

    #[test]
    fn pedindo_ou_falhou_usa_anilist() {
        let mut c = Covers::default();
        c.map.insert(4, None);
        assert_eq!(c.key(&media(4, false), true), Ok(Some("https://a/4s.jpg".into())));
        c.failed.insert(5);
        assert_eq!(c.key(&media(5, false), false), Ok(Some("https://a/5.jpg".into())));
    }
}
