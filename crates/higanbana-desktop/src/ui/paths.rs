//! Caminhos e migalhas das telas de pasta.

use higanbana_core::library::{join_path, raw_path, split_path};
use higanbana_core::nuvem::FileEntry;
use higanbana_gpui::components::{Breadcrumb, Component};
use higanbana_gpui::prelude::*;

use crate::chrome::section_label;
use crate::root::Higanbana;

/// Rota de uma subpasta: dentro de uma pasta aberta crua (`raw:`), continua crua.
pub fn folder_link(dir: &str, name: &str, raw: bool) -> String {
    let path = join_path(dir, name);
    if raw { raw_path(&path) } else { path }
}

pub fn is_comic(f: &FileEntry) -> bool {
    !f.is_directory && f.extension.as_deref().is_some_and(|e| matches!(e.to_lowercase().as_str(), ".cbz" | ".zip" | ".cbr" | ".rar"))
}

/// Caminhos dos arquivos (não pastas) de uma listagem.
pub fn file_paths(dir: &str, files: &[FileEntry]) -> Vec<String> {
    files.iter().filter(|f| !f.is_directory).map(|f| join_path(dir, &f.name)).collect()
}

/// Migalhas: Início › Seção › ... › atual; com `raw`, cada pedaço abre cru (o Início não).
pub fn breadcrumbs(cx: &mut RenderCx<'_, Higanbana>, path: &str, raw: bool) -> Element<Higanbana> {
    let segs: Vec<String> = split_path(path).into_iter().map(str::to_owned).collect();
    let go = |target: String| {
        move |v: &mut Higanbana, _: &mut EventCx| {
            v.state.go(target.clone());
        }
    };
    let mut b = Breadcrumb::new("migalhas").link("Início", go(String::new()));
    for (i, seg) in segs.iter().enumerate() {
        let label = if i == 0 { section_label(seg) } else { seg.clone() };
        b = b.link(label, go(folder_link(&segs[..i].join("/"), seg, raw)));
    }
    b.render(cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_link_keeps_raw() {
        assert_eq!(folder_link("Animes/Letra S", "Smile", false), "Animes/Letra S/Smile");
        assert_eq!(folder_link("Animes/Letra S", "Smile", true), "raw:Animes/Letra S/Smile");
        assert_eq!(folder_link("", "Animes", true), "raw:Animes");
    }
}
