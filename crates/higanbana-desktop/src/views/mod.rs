//! Telas do acervo. O estado de interface de cada tela vive em `Pages` e só
//! existe enquanto a tela está aberta: trocar de rota solta o da anterior (RAM).
//! O que precisa sobreviver (filtros e posição do catálogo) fica no `AppState`.

pub mod home;

#[cfg(test)]
mod tests;

use higanbana_core::library::{classify, Place};

#[derive(Default)]
pub struct Pages {
    route: Option<String>,
    pub home: Option<home::HomeUi>,
}

impl Pages {
    /// Garante o estado da tela de `route`; `true` se a tela mudou (o estado anterior foi solto).
    pub fn sync(&mut self, route: &str) -> bool {
        if self.route.as_deref() == Some(route) {
            return false;
        }
        *self = Pages { route: Some(route.to_owned()), ..Pages::default() };
        if let Place::Home = classify(route) {
            self.home = Some(home::HomeUi::default());
        }
        true
    }

    pub fn route(&self) -> Option<&str> {
        self.route.as_deref()
    }

    /// Estado do início (criado se faltar: handlers atrasados nunca entram em pânico).
    pub fn home(&mut self) -> &mut home::HomeUi {
        self.home.get_or_insert_default()
    }
}
