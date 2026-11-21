//! Telas do acervo. O estado de cada tela vive em `Pages` e só existe enquanto
//! a tela está aberta: trocar de rota solta o estado da anterior (RAM). O que
//! precisa sobreviver (filtros e posição do catálogo) fica no `AppState`.

#[cfg(test)]
mod tests;

/// Estado da tela aberta.
pub enum Page {
    /// Tela sem estado próprio (ou ainda não portada).
    Plain,
}

impl Page {
    fn for_route(_route: &str) -> Page {
        Page::Plain
    }
}

pub struct Pages {
    route: Option<String>,
    pub page: Page,
}

impl Default for Pages {
    fn default() -> Self {
        Pages { route: None, page: Page::Plain }
    }
}

impl Pages {
    /// Garante o estado da tela de `route`; `true` se a tela mudou (o estado anterior foi solto).
    pub fn sync(&mut self, route: &str) -> bool {
        if self.route.as_deref() == Some(route) {
            return false;
        }
        self.route = Some(route.to_owned());
        self.page = Page::for_route(route);
        true
    }

    pub fn route(&self) -> Option<&str> {
        self.route.as_deref()
    }
}
