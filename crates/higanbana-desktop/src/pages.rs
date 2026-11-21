//! Tela da rota atual (`library::classify`). Telas ainda não portadas mostram
//! um aviso provisório com o caminho.

use higanbana_core::library::{classify, Place};
use higanbana_gpui::prelude::*;

use crate::chrome::section_label;
use crate::root::Higanbana;
use crate::ui::empty_state;

/// Catálogo e pasta têm a própria rolagem (lista virtual).
pub fn own_scroll(route: &str) -> bool {
    matches!(classify(route), Place::Catalog { .. } | Place::Folder { .. } | Place::Raw { .. })
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let path = v.state.path().to_owned();
    match classify(&path) {
        Place::Home => crate::views::home::page(v, cx),
        Place::Catalog { .. } => crate::views::catalog::page(v, cx),
        Place::Title { title_path, rest, media, .. } => crate::views::title::page(v, cx, &title_path, &rest, media),
        _ => placeholder(&path, cx),
    }
}

fn placeholder(path: &str, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let title = if path.is_empty() { "Início".to_owned() } else { path.split('/').map(section_label).collect::<Vec<_>>().join(" / ") };
    empty_state(cx.theme(), "compass", title, Some("Esta tela ainda está sendo migrada para a nova interface.".into()))
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
