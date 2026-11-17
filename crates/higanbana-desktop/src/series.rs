//! Catálogo de séries como componente único: Animes e Mangás usam a mesma tela
//! (`views::catalog`), o mesmo estado (`catalog_state`) e a mesma barra de filtros.
//! O que muda entre eles fica aqui. Uma seção nova (Doramas, por exemplo) entra
//! com um descritor novo e um índice de letras no núcleo.

use higanbana_core::{anilist::MediaType, nuvem::SearchKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Series {
    pub media: MediaType,
    /// Pasta raiz na nuvem (`Animes`, `Mangás/Séries Mangás`).
    pub root: &'static str,
    /// Título da tela.
    pub title: &'static str,
    /// Tipo dos resultados da busca da nuvem que pertencem a esta seção.
    pub search_kind: SearchKind,
    /// Filtros que só fazem sentido para vídeo.
    pub kind_filter: bool,
    pub season_filter: bool,
    pub dubbed_filter: bool,
}

pub const ANIME: Series = Series {
    media: MediaType::Anime,
    root: higanbana_core::anime_index::ANIME_ROOT,
    title: "Animes",
    search_kind: SearchKind::Anime,
    kind_filter: true,
    season_filter: true,
    dubbed_filter: true,
};

pub const MANGA: Series = Series {
    media: MediaType::Manga,
    root: higanbana_core::anime_index::MANGA_ROOT,
    title: "Mangás",
    search_kind: SearchKind::Manga,
    kind_filter: false,
    season_filter: false,
    dubbed_filter: false,
};

impl Series {
    pub fn of(media: MediaType) -> Self {
        match media {
            MediaType::Anime => ANIME,
            MediaType::Manga => MANGA,
        }
    }

    /// Prefixo dos ids dos cards (único por seção).
    pub fn id_prefix(&self) -> &'static str {
        match self.media {
            MediaType::Anime => "anime",
            MediaType::Manga => "manga",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use higanbana_core::library::catalog_root;

    #[test]
    fn roots_match_core() {
        for media in higanbana_core::library::CATALOG_MEDIA {
            assert_eq!(Series::of(media).root, catalog_root(media));
            assert_eq!(Series::of(media).media, media);
        }
    }
}
