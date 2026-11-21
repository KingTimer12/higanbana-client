//! Estante: pasta cujos filhos são títulos → grade de pôsteres com filtro,
//! índice de letras e coleções irmãs. Grade em lista virtual (linhas da grade).

use std::sync::Arc;

use higanbana_core::anilist::MediaType;
use higanbana_core::library::{base_name, join_path, natural_cmp, parent_path};
use higanbana_core::nuvem::FileEntry;
use higanbana_gpui::components::{Button, Component, ComponentSize, Input, Skeleton, Variant, VirtualList};
use higanbana_gpui::prelude::*;

use crate::root::Higanbana;
use crate::state::Listing;
use crate::ui::cover::{columns, content_width, title_card};
use crate::ui::empty_state;
use crate::ui::paths::breadcrumbs;

const GAP: f32 = 20.0;

pub fn collection_label(name: &str) -> String {
    let inner = name.trim_start_matches('[').trim_end_matches(']');
    let inner = inner.strip_prefix("Colecao ").or_else(|| inner.strip_prefix("Coleção ")).unwrap_or(inner);
    inner.strip_prefix("de ").unwrap_or(inner).to_owned()
}

pub fn is_index_letter(name: &str) -> bool {
    name == "0-9" || name.strip_prefix("Letra ").is_some_and(|l| l.chars().count() == 1)
}

/// Títulos (subpastas) que passam no filtro, em ordem natural.
pub fn titles(files: &[FileEntry], query: &str) -> Vec<FileEntry> {
    let q = query.trim().to_lowercase();
    let mut t: Vec<FileEntry> = files.iter().filter(|f| f.is_directory && (q.is_empty() || f.name.to_lowercase().contains(&q))).cloned().collect();
    t.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    t
}

#[derive(Default)]
pub struct ShelfUi {
    pub filter: TextInputState,
}

fn header(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, path: &str, count: Option<usize>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let name = base_name(path).to_owned();
    let parent = parent_path(path);
    let filter = Input::new("estante-filtro", &v.pages.shelf().filter, |v: &mut Higanbana| &mut v.pages.shelf().filter).width(288.0).prefix("search").on_change(|_: &mut Higanbana, _, cx| cx.notify()).render(cx);
    let mut names = div().flex_col().gap(4.0).child(text(name.clone()).text_size(t.typography.xxl).font_weight(600));
    if let Some(n) = count {
        names = names.child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(format!("{n} títulos"))));
    }
    let mut col = div().id("estante-cabecalho").track_bounds().flex_col().pt(24.0).gap(24.0).child(breadcrumbs(cx, path, false)).child(div().flex_wrap().items_end().justify_between().gap(16.0).child(names).child(filter));
    let siblings: Vec<FileEntry> = match v.state.listing(&parent) {
        Listing::Ready(f) => f.iter().filter(|f| f.is_directory).cloned().collect(),
        _ => Vec::new(),
    };
    let accent = t.accent;
    let jump = |cx: &mut RenderCx<'_, Higanbana>, s: &FileEntry, label: String| {
        let target = join_path(&parent, &s.name);
        let b = Button::new(("irma", s.name.as_str()), label).variant(Variant::Ghost).size(ComponentSize::Compact).on_click(move |v: &mut Higanbana, _, cx| {
            v.state.go(target.clone());
            cx.notify();
        });
        let b = b.render(cx);
        if s.name == name { b.bg(accent) } else { b }
    };
    let (letters, collections): (Vec<&FileEntry>, Vec<&FileEntry>) = siblings.iter().partition(|f| is_index_letter(&f.name));
    let mut nav = div().flex_col().gap(8.0);
    if !letters.is_empty() {
        let mut row = div().flex_wrap().gap(2.0);
        for s in letters {
            row = row.child(jump(cx, s, s.name.trim_start_matches("Letra ").to_owned()));
        }
        nav = nav.child(row);
    }
    if !collections.is_empty() {
        let mut row = div().flex_wrap().items_center().gap(4.0).child(div().mr(8.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(text("Coleções")));
        for s in collections {
            row = row.child(jump(cx, s, collection_label(&s.name)));
        }
        nav = nav.child(row);
    }
    col = col.child(nav);
    col
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, path: &str, media: MediaType) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let width = content_width(v, cx, 1280.0);
    let cols = columns(width, 160.0, GAP);
    let card_w = ((width - (cols as f32 - 1.0) * GAP) / cols as f32).floor();
    let listing = v.state.listing(path);
    let query = v.pages.shelf().filter.text().to_owned();
    let (count, list) = match &listing {
        Listing::Ready(f) => (Some(f.iter().filter(|f| f.is_directory).count()), Some(titles(f, &query))),
        _ => (None, None),
    };
    let header_h = cx.bounds("estante-cabecalho").map_or(200.0, |r| r.size.height);
    let row_h = card_w * 1.5 + 8.0 + 40.0 + 18.0 + 32.0;
    let n_rows = list.as_ref().map_or(0, |l| l.len().div_ceil(cols));
    let heights: Arc<[f32]> = std::iter::once(header_h).chain(std::iter::repeat_n(row_h, n_rows.max(1))).collect();
    let path_owned = path.to_owned();
    let list = list.map(Arc::new);
    let body = VirtualList::new(("estante", path), heights).render(cx, |i, cx| {
        let inner = if i == 0 {
            header(v, cx, &path_owned, count)
        } else {
            match (&listing, &list) {
                (Listing::Failed(e), _) => empty_state(&t, "wifi-off", "Não foi possível abrir esta pasta", Some(e.clone().into())),
                (_, Some(l)) if l.is_empty() => empty_state(&t, "search", format!("Nenhum título com “{}” em {}", query.trim(), base_name(&path_owned)), None),
                (_, Some(l)) => {
                    let mut row = div().gap(GAP).pt(32.0).items_start();
                    for e in l.iter().skip((i - 1) * cols).take(cols) {
                        row = row.child(title_card(v, cx, "title", &e.name, &join_path(&path_owned, &e.name), media, card_w));
                    }
                    row
                }
                _ => {
                    let mut row = div().gap(GAP).pt(32.0);
                    for k in 0..cols {
                        row = row.child(Skeleton::new(("estante-esqueleto", k)).w(card_w).h(card_w * 1.5).render(cx).rounded(t.radius_lg));
                    }
                    row
                }
            }
        };
        div().w_full().flex_col().items_center().child(div().w(width).flex_col().child(inner))
    });
    div().size_full().child(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_names_without_the_prefix() {
        assert_eq!(collection_label("[Colecao de Animes com Dublagem Portuguesa]"), "Animes com Dublagem Portuguesa");
        assert_eq!(collection_label("[Colecao Curtas]"), "Curtas");
        assert_eq!(collection_label("Extras"), "Extras");
    }

    #[test]
    fn index_letters_apart_from_collections() {
        assert!(is_index_letter("Letra C"));
        assert!(is_index_letter("0-9"));
        assert!(!is_index_letter("[Colecao Curtas]"));
        assert!(!is_index_letter("Letra Grande"));
    }

    #[test]
    fn estante_busca_filtra() {
        let d = |n: &str| FileEntry { name: n.into(), is_directory: true, size: 0, modified: String::new(), extension: None };
        let files = vec![d("Goblin"), d("Crash Landing on You"), d("goblin 2")];
        let found: Vec<String> = titles(&files, " GOB ").into_iter().map(|f| f.name).collect();
        assert_eq!(found, ["Goblin", "goblin 2"]);
    }
}
