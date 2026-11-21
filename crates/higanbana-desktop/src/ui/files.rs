//! Lista de arquivos de uma pasta: subpastas primeiro, depois arquivos como
//! episódios (número, nome e detalhes, ações). Serve à lista comum e à virtual.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use higanbana_core::library::{join_path, natural_cmp, parse_episode};
use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::logic::VIDEO_EXT;
use higanbana_core::reader::store::ReadProgress;
use higanbana_gpui::components::{Button, Component, ComponentSize, Progress, ProgressSize, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{Corners, ElementId};

use super::download::download_button;
use super::hover;
use super::paths::{folder_link, is_comic};
use super::{empty_state, truncate};
use crate::chrome::format_bytes;
use crate::root::Higanbana;
use crate::theme::primary_ink;

/// Ação de arquivo (assistir, ler).
pub type FileAction = Rc<dyn Fn(&mut Higanbana, &FileEntry, &mut EventCx)>;

/// Progresso de leitura por caminho.
pub type ReadingMap = HashMap<String, ReadProgress>;

/// Altura das linhas na lista virtual: pasta e arquivo.
pub const DIR_ROW: f32 = 48.0;
pub const FILE_ROW: f32 = 72.0;

pub struct FileRows {
    dir: String,
    raw: bool,
    /// Pastas primeiro, depois arquivos, em ordem natural (compartilhado, sem cópia por frame).
    entries: Arc<[FileEntry]>,
    /// Qualidade comum a todos os episódios (mostrada uma vez no topo).
    shared: Option<String>,
    on_play: Option<FileAction>,
    on_read: Option<FileAction>,
}

/// Ordena (pastas primeiro, ordem natural) — feito uma vez por listagem.
pub fn sorted(files: &[FileEntry]) -> Arc<[FileEntry]> {
    let mut v = files.to_vec();
    v.sort_by(|a, b| b.is_directory.cmp(&a.is_directory).then_with(|| natural_cmp(&a.name, &b.name)));
    v.into()
}

/// Qualidade comum quando todos os episódios (2+) têm a mesma.
pub fn shared_quality(entries: &[FileEntry]) -> Option<String> {
    let eps: Vec<_> = entries.iter().filter(|f| !f.is_directory).map(|f| parse_episode(&f.name)).filter(|e| e.number.is_some()).collect();
    (eps.len() >= 2).then(|| eps[0].quality.clone()).flatten().filter(|q| eps.iter().all(|e| e.quality.as_deref() == Some(q)))
}

impl FileRows {
    /// `entries` já ordenado por [`sorted`].
    pub fn new(dir: &str, entries: Arc<[FileEntry]>, raw: bool, on_play: Option<FileAction>, on_read: Option<FileAction>) -> Self {
        let shared = shared_quality(&entries);
        Self { dir: dir.to_owned(), raw, entries, shared, on_play, on_read }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Progresso de leitura, só onde há o que ler.
    pub fn reading(&self, v: &Higanbana) -> ReadingMap {
        if self.on_read.is_some() { v.state.core.reader().store.data().progress } else { ReadingMap::default() }
    }

    pub fn heights(&self) -> Arc<[f32]> {
        self.entries.iter().map(|f| if f.is_directory { DIR_ROW } else { FILE_ROW }).collect()
    }

    /// "Todos os episódios em 1080p", quando vale.
    pub fn shared_note(&self, t: &Theme) -> Option<Element<Higanbana>> {
        self.shared.as_ref().map(|q| div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(format!("Todos os episódios em {q}"))))
    }

    /// Uma linha; `fixed` fixa a altura (lista virtual).
    pub fn row(&self, v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, i: usize, reading: &ReadingMap, fixed: bool) -> Element<Higanbana> {
        let t = cx.theme().clone();
        let f = &self.entries[i];
        let n = self.entries.len();
        let path = join_path(&self.dir, &f.name);
        let r = t.radius_lg;
        let radius = Corners { top_left: if i == 0 { r } else { 0.0 }, top_right: if i == 0 { r } else { 0.0 }, bottom_left: if i + 1 == n { r } else { 0.0 }, bottom_right: if i + 1 == n { r } else { 0.0 } };
        let id = ElementId::from(("linha", f.name.as_str()));
        let bg = hover::layer(hover::amount(cx, id), radius, Some(t.list_hover), None);
        let mut row = div().relative().items_center().gap(12.0).px(12.0).py(10.0).child(bg);
        if fixed {
            row = row.h(if f.is_directory { DIR_ROW } else { FILE_ROW });
        }
        if i > 0 {
            row = row.child(div().absolute().top(0.0).left(0.0).right(0.0).h(1.0).bg(t.border));
        }
        if f.is_directory {
            let target = folder_link(&self.dir, &f.name, self.raw);
            let row = row
                .cursor_pointer()
                .child(div().w(40.0).flex_shrink_0().justify_center().child(icon("folder").size(16.0).text_color(t.muted_foreground)))
                .child(truncate(f.name.clone()).flex_1().text_size(t.typography.sm))
                .child(icon("chevron-right").size(14.0).text_color(t.muted_foreground))
                .on_click(move |v: &mut Higanbana, _, cx| {
                    v.state.go(target.clone());
                    cx.notify();
                });
            return hover::track(row, id);
        }
        let ep = parse_episode(&f.name);
        let label = match (&ep.number, &self.shared) {
            (None, _) => f.name.clone(),
            (Some(n), Some(_)) => format!("Episódio {}", n.parse::<f64>().map_or_else(|_| n.clone(), |v| v.to_string())),
            (Some(_), None) => ep.quality.clone().unwrap_or_else(|| ep.label.clone()),
        };
        let ext = f.extension.clone().unwrap_or_default().to_lowercase();
        let playable = self.on_play.is_some() && VIDEO_EXT.contains(&ext.as_str());
        let readable = self.on_read.is_some() && is_comic(f);
        let read_p = readable.then(|| reading.get(&path).cloned()).flatten();
        let seen = v.state.player_data.progress.get(&path).map(|p| (p.watched, p.position / p.duration.max(1.0)));
        let (watched, frac) = match (&read_p, seen) {
            (Some(r), _) => (r.read, (r.page as f64 + 1.0) / (r.total.max(1) as f64)),
            (None, Some(p)) => p,
            _ => (false, -1.0),
        };
        let details = [ext.trim_start_matches('.').to_uppercase(), format_bytes(f.size)].join(" · ");
        let primary = if playable { self.on_play.clone() } else if readable { self.on_read.clone() } else { None };
        if let Some(act) = primary {
            let f2 = f.clone();
            row = row.cursor_pointer().on_click(move |v: &mut Higanbana, _, cx| act(v, &f2, cx));
        }
        let number = match &ep.number {
            Some(_) if watched => icon("check").size(16.0).text_color(t.success_foreground),
            Some(n) => div().text_color(if frac >= 0.0 { primary_ink(&t) } else { t.muted_foreground }).child(text(n.clone())),
            None => icon("file-play").size(16.0).text_color(t.muted_foreground),
        };
        let mut meta = div().gap(12.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(text(details));
        if watched {
            meta = meta.child(div().gap(4.0).items_center().text_color(t.success_foreground).child(icon("check").size(12.0)).child(text(if readable { "Lido" } else { "Visto" })));
        }
        let mut info = div().flex_col().flex_1().min_w(0.0).gap(2.0).child(truncate(label).text_size(t.typography.sm).font_weight(500)).child(meta);
        if frac >= 0.0 && !watched {
            let bar = Progress::new(("visto", f.name.as_str())).size(ProgressSize::XSmall).value((frac.min(1.0) * 100.0) as f32).render(cx);
            info = info.child(div().pt(4.0).w(192.0).child(bar));
        }
        row = row.child(div().w(40.0).flex_shrink_0().justify_center().text_size(t.typography.lg).font_weight(600).child(number)).child(info);
        // Botões dentro da linha: o clique fica no mais interno (não toca o episódio).
        if playable {
            let (act, f2) = (self.on_play.clone().expect("playable"), f.clone());
            let b = Button::new(("assistir", f.name.as_str()), "Assistir").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("play").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| act(v, &f2, cx));
            row = row.child(b.render(cx));
        }
        if readable {
            let (act, f2) = (self.on_read.clone().expect("readable"), f.clone());
            let label = if read_p.as_ref().is_some_and(|r| !r.read) { "Continuar" } else { "Ler" };
            let b = Button::new(("ler", f.name.as_str()), label).variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("book-open").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| act(v, &f2, cx));
            row = row.child(b.render(cx));
        }
        let dl = download_button(v, cx, ("baixar", f.name.as_str()), &path);
        row = row.child(div().w(80.0).flex_shrink_0().justify_end().child(dl));
        hover::track(row, id)
    }
}

/// Moldura da lista (borda e fundo do grupo).
pub fn frame(t: &Theme) -> Element<Higanbana> {
    div().flex_col().rounded(t.radius_lg).border(1.0, t.border).bg(t.group_box).overflow_hidden()
}

pub fn empty_folder(t: &Theme) -> Element<Higanbana> {
    empty_state(t, "folder-open", "Esta pasta está vazia", None).rounded(t.radius_lg).border(1.0, t.border)
}

/// Lista comum (sem virtualizar), para pastas pequenas dentro de outras telas.
pub fn file_list(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, rows: &FileRows) -> Element<Higanbana> {
    let t = cx.theme().clone();
    if rows.is_empty() {
        return empty_folder(&t);
    }
    let reading = rows.reading(v);
    let mut list = frame(&t);
    for i in 0..rows.len() {
        list = list.child(rows.row(v, cx, i, &reading, false));
    }
    let mut col = div().flex_col().gap(8.0);
    if let Some(n) = rows.shared_note(&t) {
        col = col.child(n);
    }
    col.child(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, dir: bool) -> FileEntry {
        FileEntry { name: name.into(), is_directory: dir, size: 0, modified: String::new(), extension: None }
    }

    #[test]
    fn pastas_primeiro_em_ordem_natural() {
        let s = sorted(&[file("ep 10.mkv", false), file("Extras", true), file("ep 2.mkv", false)]);
        let names: Vec<_> = s.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Extras", "ep 2.mkv", "ep 10.mkv"]);
    }

    #[test]
    fn alturas_por_tipo() {
        let rows = FileRows::new("x", sorted(&[file("a", true), file("b.mkv", false)]), false, None, None);
        assert_eq!(&*rows.heights(), &[DIR_ROW, FILE_ROW]);
    }
}
