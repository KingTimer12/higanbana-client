//! Telas do acervo. O estado de interface de cada tela vive em `Pages` e só
//! existe enquanto a tela está aberta: trocar de rota solta o da anterior (RAM).
//! O que precisa sobreviver (filtros e posição do catálogo) fica no `AppState`.

pub mod catalog;
pub mod franchise;
pub mod home;
pub mod title;

#[cfg(test)]
mod tests;

use higanbana_core::anilist::MediaType;
use higanbana_core::library::{classify, Place};

use crate::state::{AppState, CatalogChoice};

#[derive(Default)]
pub struct Pages {
    route: Option<String>,
    pub home: Option<home::HomeUi>,
    pub catalog: Option<catalog::CatalogUi>,
    pub title: Option<title::TitleUi>,
}

impl Pages {
    /// Garante o estado da tela de `route`; `true` se a tela mudou (o estado anterior foi solto).
    /// `saved` dá o que o catálogo de uma mídia tinha da última vez.
    pub fn sync(&mut self, route: &str, saved: impl Fn(MediaType) -> CatalogChoice) -> bool {
        if self.route.as_deref() == Some(route) {
            return false;
        }
        *self = Pages { route: Some(route.to_owned()), ..Pages::default() };
        match classify(route) {
            Place::Home => self.home = Some(home::HomeUi::default()),
            Place::Catalog { media, letter } => self.catalog = Some(catalog::CatalogUi::new(&saved(media), media, letter)),
            _ => {}
        }
        true
    }

    /// Mídia do catálogo aberto (pela rota).
    pub fn catalog_media(&self) -> MediaType {
        match self.route.as_deref().map(classify) {
            Some(Place::Catalog { media, .. }) => media,
            _ => self.catalog.as_ref().map_or(MediaType::Anime, |c| c.series.media),
        }
    }

    /// Estado do catálogo (criado se faltar).
    pub fn catalog(&mut self, state: &AppState, media: MediaType) -> &mut catalog::CatalogUi {
        self.catalog.get_or_insert_with(|| catalog::CatalogUi::new(state.catalog_choice(media), media, None))
    }

    pub fn route(&self) -> Option<&str> {
        self.route.as_deref()
    }

    /// Estado do título (criado se faltar).
    pub fn title(&mut self) -> &mut title::TitleUi {
        self.title.get_or_insert_default()
    }

    /// Estado do início (criado se faltar: handlers atrasados nunca entram em pânico).
    pub fn home(&mut self) -> &mut home::HomeUi {
        self.home.get_or_insert_default()
    }
}
