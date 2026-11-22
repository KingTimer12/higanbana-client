//! Tela da rota atual (`library::classify`).

use higanbana_core::library::{classify, Place};
use higanbana_gpui::prelude::*;

use crate::root::Higanbana;

/// Catálogo e pasta têm a própria rolagem (lista virtual).
pub fn own_scroll(route: &str) -> bool {
    matches!(classify(route), Place::Catalog { .. } | Place::Folder { .. } | Place::Raw { .. } | Place::Shelf { .. })
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let path = v.state.path().to_owned();
    match classify(&path) {
        Place::Home => crate::views::home::page(v, cx),
        Place::Catalog { .. } => crate::views::catalog::page(v, cx),
        Place::Title { title_path, rest, media, .. } => crate::views::title::page(v, cx, &title_path, &rest, media),
        Place::Folder { path } => crate::views::folder::page(v, cx, &path, false),
        Place::Raw { path } => crate::views::folder::page(v, cx, &path, true),
        Place::Shelf { path, media } => crate::views::shelf::page(v, cx, &path, media),
    }
}


#[cfg(test)]
mod tests {
    use super::own_scroll;

    #[test]
    fn rolagem_propria_no_catalogo_e_pasta() {
        assert!(own_scroll("Animes"));
        assert!(own_scroll("raw:Animes/Letra F"));
        assert!(own_scroll("Doramas/Coreia"));
        assert!(!own_scroll(""));
    }
}
