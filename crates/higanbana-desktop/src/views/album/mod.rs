//! Encarte do álbum ("encarte de CD japonês"): capa com obi, faixas por disco e
//! a aba de arquivos que não são faixas.

mod cover;
mod tracks;

use std::rc::Rc;

use higanbana_core::music::album::Album;
use higanbana_gpui::components::{Component, TabBar};
use higanbana_gpui::prelude::*;

use crate::root::Higanbana;
use crate::state::album::AlbumLoad;
use crate::ui::files::{file_list, sorted, FileRows};

#[derive(Debug, PartialEq)]
pub enum Body {
    Album,
    Files,
    Loading,
}

/// O que a pasta mostra: encarte, lista de arquivos ou esqueleto.
pub fn body_for(looks_album: bool, load: Option<&AlbumLoad>) -> Body {
    match (looks_album, load) {
        (false, _) | (true, Some(AlbumLoad::NotAlbum)) => Body::Files,
        (true, Some(AlbumLoad::Ready(_))) => Body::Album,
        (true, _) => Body::Loading,
    }
}

/// Faixas (índices) de cada disco, na ordem dos discos.
pub fn disc_groups(a: &Album) -> Vec<(u32, Vec<usize>)> {
    a.discs.iter().map(|&d| (d, a.tracks.iter().enumerate().filter(|(_, t)| t.disc == d).map(|(i, _)| i).collect())).collect()
}

/// Encarte (`a` já carregado) na largura `width`.
pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, a: &Rc<Album>, work: Option<String>) -> Element<Higanbana> {
    let files_tab = v.state.album.as_ref().is_some_and(|s| s.files_tab);
    let aside = cover::aside(v, cx, a, work);
    let others = a.others.len();
    let tabs = TabBar::new("album-abas", usize::from(files_tab), |v: &mut Higanbana, i, cx| {
        if let Some(s) = v.state.album.as_mut() {
            s.files_tab = i == 1;
        }
        cx.notify();
    })
    .underline()
    .tab("Faixas")
    .tab(format!("Arquivos {others}"))
    .render(cx);
    let body = if files_tab {
        let t = cx.theme().clone();
        let rows = FileRows::new(&a.path, sorted(&a.others), false, None, None);
        div().flex_col().gap(12.0).child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text("Arquivos desta pasta que não são faixas."))).child(file_list(v, cx, &rows))
    } else {
        tracks::tracks(v, cx, a)
    };
    div().w_full().items_start().gap(32.0).child(aside).child(div().flex_1().min_w(0.0).flex_col().gap(16.0).child(tabs).child(body))
}

#[cfg(test)]
mod tests;
