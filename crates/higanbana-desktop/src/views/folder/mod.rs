//! Pasta comum (ou aberta crua): lista virtual de arquivos com o cabeçalho fixo;
//! pasta só de subpastas curtas (letras, coleções) vira uma grade de blocos.

use std::sync::Arc;

use higanbana_core::library::{base_name, natural_cmp, split_path};
use higanbana_core::nuvem::FileEntry;
use higanbana_gpui::components::{Component, Skeleton, VirtualList};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{Corners, ElementId};

use crate::chrome::section_label;
use crate::reader::ReadTarget;
use crate::root::Higanbana;
use crate::state::Listing;
use crate::ui::cover::{columns, content_width};
use crate::ui::files::{empty_folder, frame, sorted, FileAction, FileRows};
use crate::ui::paths::{breadcrumbs, folder_link};
use crate::ui::{empty_state, hover};
use crate::video::PlayTarget;

/// Pasta que é só um índice (letras, coleções): 6+ subpastas de nome curto e nada mais.
pub fn is_index(files: &[FileEntry]) -> bool {
    let dirs = files.iter().filter(|f| f.is_directory).count();
    dirs >= 6 && dirs == files.len() && files.iter().all(|d| d.name.chars().count() <= 16)
}

/// Título do anime para o player: a pasta de cima quando esta é temporada/release.
pub fn anime_title(path: &str) -> String {
    let segs = split_path(path);
    let last = segs.last().copied().unwrap_or_default();
    let lower = last.to_lowercase();
    match segs.len().checked_sub(2).map(|i| segs[i]) {
        Some(up) if lower.starts_with("temporada") || lower.starts_with("season") || last.starts_with('[') || last.starts_with('(') => up.to_owned(),
        _ => last.to_owned(),
    }
}

/// Listagem ordenada em cache (a mesma `Arc` da listagem não é reordenada a cada quadro).
#[derive(Default)]
pub struct FolderUi {
    sorted: Option<(Arc<[FileEntry]>, Arc<[FileEntry]>)>,
}

fn sorted_cached(v: &mut Higanbana, files: &Arc<[FileEntry]>) -> Arc<[FileEntry]> {
    let ui = v.pages.folder();
    if let Some((src, s)) = &ui.sorted
        && Arc::ptr_eq(src, files)
    {
        return s.clone();
    }
    let s = sorted(files);
    ui.sorted = Some((files.clone(), s.clone()));
    s
}

fn index_grid(cx: &mut RenderCx<'_, Higanbana>, path: &str, raw: bool, files: &[FileEntry], width: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let mut names: Vec<&str> = files.iter().map(|d| d.name.as_str()).collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    let gap = 12.0;
    let cols = columns(width, 96.0, gap);
    let side = ((width - (cols as f32 - 1.0) * gap) / cols as f32).floor();
    let mut grid = div().flex_wrap().gap(gap);
    for name in names {
        let short = name.trim_start_matches("Letra ").trim_start_matches('[').trim_end_matches(']').to_owned();
        let big = short.chars().count() <= 3;
        let target = folder_link(path, name, raw);
        let id = ElementId::from(("bloco", name));
        let ring = hover::layer(hover::amount(cx, id), Corners::all(t.radius_lg), None, Some(t.ring));
        let tile = div()
            .relative()
            .w(side)
            .h(side)
            .items_center()
            .justify_center()
            .p(8.0)
            .rounded(t.radius_lg)
            .border(1.0, t.border)
            .bg(t.group_box)
            .font_weight(600)
            .text_size(if big { t.typography.xxl } else { t.typography.sm })
            .cursor_pointer()
            .child(text(short))
            .child(ring)
            .on_click(move |v: &mut Higanbana, _, cx| {
                v.state.go(target.clone());
                cx.notify();
            });
        grid = grid.child(hover::track(tile, id));
    }
    grid
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, path: &str, raw: bool) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let width = content_width(v, cx, 1280.0);
    let segs = split_path(path);
    let title = if segs.len() == 1 { section_label(segs[0]) } else { segs.last().map(|s| s.to_string()).unwrap_or_default() };
    let header = div().flex_col().gap(24.0).child(breadcrumbs(cx, path, raw)).child(text(title.clone()).text_size(t.typography.xxl).font_weight(600));
    let page = div().w(width).h_full().flex_col().py(24.0).gap(24.0).child(header);
    let wrap = |e: Element<Higanbana>| div().size_full().flex_col().items_center().child(e);
    let files = match v.state.listing(path) {
        Listing::Failed(e) => return wrap(page.child(empty_state(&t, "wifi-off", "Não foi possível abrir esta pasta", Some(e.into())))),
        Listing::Loading => return wrap(page.child(Skeleton::new("pasta-esqueleto").w(pct(100.0)).h(288.0).render(cx).rounded(t.radius_lg))),
        Listing::Ready(f) => f,
    };
    if is_index(&files) {
        return scroll(("pasta-rolagem", path)).size_full().items_center().child(div().w(width).flex_col().py(24.0).gap(24.0).child(breadcrumbs(cx, path, raw)).child(text(title).text_size(t.typography.xxl).font_weight(600)).child(index_grid(cx, path, raw, &files, width)));
    }
    let entries = sorted_cached(v, &files);
    let (dir, all, name) = (path.to_owned(), files.clone(), anime_title(path));
    let on_play: FileAction = std::rc::Rc::new(move |v: &mut Higanbana, f: &FileEntry, cx: &mut EventCx| {
        v.state.play(PlayTarget { dir: dir.clone(), file: f.clone(), files: all.to_vec(), mal_id: None, is_anime: false, anime_title: name.clone() });
        cx.notify();
    });
    let (read_dir, read_files, read_title) = (path.to_owned(), files.clone(), base_name(path).to_owned());
    let on_read: FileAction = std::rc::Rc::new(move |v: &mut Higanbana, f: &FileEntry, cx: &mut EventCx| {
        v.state.read(ReadTarget { dir: read_dir.clone(), file: f.clone(), files: read_files.clone(), title_path: read_dir.clone(), title: read_title.clone() });
        cx.notify();
    });
    let rows = FileRows::new(path, entries, raw, Some(on_play), Some(on_read));
    if rows.is_empty() {
        return wrap(page.child(empty_folder(&t)));
    }
    let reading = rows.reading(v);
    let heights = rows.heights();
    let list = VirtualList::new(("pasta-arquivos", path), heights).render(cx, |i, cx| rows.row(v, cx, i, &reading, true));
    let mut page = page;
    if let Some(n) = rows.shared_note(&t) {
        page = page.child(n);
    }
    wrap(page.child(frame(&t).flex_1().min_h(0.0).child(list)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> FileEntry {
        FileEntry { name: name.into(), is_directory: true, size: 0, modified: String::new(), extension: None }
    }

    #[test]
    fn pasta_de_letras_vira_indice() {
        let letters: Vec<FileEntry> = ["0-9", "Letra A", "Letra B", "Letra C", "Letra D", "Letra E"].map(dir).to_vec();
        assert!(is_index(&letters));
        assert!(!is_index(&letters[..5]));
    }

    #[test]
    fn titulo_do_anime_sobe_da_temporada() {
        assert_eq!(anime_title("Doramas/Coreia/Goblin/Temporada 1"), "Goblin");
        assert_eq!(anime_title("Doramas/Coreia/Goblin"), "Goblin");
    }

    #[test]
    fn pasta_vazia_mostra_empty_folder() {
        let rows = FileRows::new("x", sorted(&[]), false, None, None);
        assert!(rows.is_empty());
    }

    #[test]
    fn pasta_grande_virtualiza() {
        let files: Vec<FileEntry> = (0..2000).map(|i| FileEntry { name: format!("ep {i}.mkv"), is_directory: false, size: 0, modified: String::new(), extension: Some(".mkv".into()) }).collect();
        let rows = FileRows::new("x", sorted(&files), false, None, None);
        let h = rows.heights();
        let r = higanbana_gpui::components::visible_range(&h, 0.0, 0.0, 600.0);
        assert!(r.end - r.start < 40, "só as linhas visíveis: {}", r.end - r.start);
    }
}
