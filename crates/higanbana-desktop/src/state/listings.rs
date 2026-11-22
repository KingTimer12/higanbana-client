//! Listagens de pasta da nuvem, por caminho. Só a pasta aberta pelo usuário é
//! listada (nada de varrer). Ao navegar, as listagens que não são da tela nova
//! saem da memória; resposta que chega para uma pasta já deixada é descartada.

use std::collections::HashMap;
use std::sync::Arc;

use higanbana_core::nuvem::FileEntry;

use super::AppState;

#[derive(Clone)]
pub enum Listing {
    Loading,
    Ready(Arc<[FileEntry]>),
    Failed(String),
}

#[derive(Default)]
pub(super) struct Listings {
    map: HashMap<String, Listing>,
}

impl Listings {
    /// `true` se começou agora (precisa pedir).
    fn start(&mut self, path: &str) -> bool {
        if self.map.contains_key(path) {
            return false;
        }
        self.map.insert(path.to_owned(), Listing::Loading);
        true
    }

    /// Guarda a resposta só se a pasta ainda é esperada. `true` se guardou.
    fn finish(&mut self, path: &str, result: Result<Vec<FileEntry>, String>) -> bool {
        let Some(slot) = self.map.get_mut(path) else { return false };
        *slot = match result {
            Ok(files) => Listing::Ready(files.into()),
            Err(e) => Listing::Failed(e),
        };
        true
    }

    /// Solta tudo que não está dentro de `root` (a pasta da tela ou do título).
    fn retain(&mut self, root: Option<&str>) {
        self.map.retain(|k, _| root.is_some_and(|r| within(k, r)));
    }

    fn get(&self, path: &str) -> Option<Listing> {
        self.map.get(path).cloned()
    }
}

/// `path` é `root` ou fica dentro dele.
fn within(path: &str, root: &str) -> bool {
    root.is_empty() || path == root || path.strip_prefix(root).is_some_and(|r| r.starts_with('/'))
}

/// Pasta cujas listagens a tela de `route` usa: o título inteiro (versões,
/// temporadas) ou a própria pasta; `None` = a tela não lista pastas.
pub fn screen_root(route: &str) -> Option<String> {
    use higanbana_core::library::{classify, Place};
    match classify(route) {
        Place::Title { title_path, .. } => Some(title_path),
        Place::Folder { path } | Place::Raw { path } | Place::Shelf { path, .. } => Some(path),
        Place::Home | Place::Catalog { .. } => None,
    }
}

impl AppState {
    /// Listagem de `path` (caminho da nuvem); começa a pedir na primeira vez.
    pub fn listing(&mut self, path: &str) -> Listing {
        if self.listings.start(path) {
            let (core, p) = (self.core.clone(), path.to_owned());
            let key = p.clone();
            self.run(async move { core.cloud().list(&p).await.map(|l| l.files).map_err(|e| e.to_string()) }, move |s, r, cx| {
                if s.listings.finish(&key, r) {
                    cx.notify();
                }
            });
        }
        self.listings.get(path).unwrap_or(Listing::Loading)
    }

    /// Esquece a listagem de `path` (tentar de novo).
    pub fn forget_listing(&mut self, path: &str) {
        self.listings.map.remove(path);
    }

    /// Mantém só as listagens dentro de `root` (chamado ao trocar de tela).
    pub fn keep_listings(&mut self, root: Option<&str>) {
        self.listings.retain(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str) -> FileEntry {
        FileEntry { name: name.into(), is_directory: false, size: 0, modified: String::new(), extension: None }
    }

    #[test]
    fn listagem_antiga_descartada() {
        let mut l = Listings::default();
        assert!(l.start("A"));
        l.retain(Some("B"));
        assert!(l.start("B"));
        assert!(!l.finish("A", Ok(vec![f("velho")])), "A já foi deixada");
        assert!(l.get("A").is_none());
        assert!(matches!(l.get("B"), Some(Listing::Loading)));
    }

    #[test]
    fn pedido_unico_por_pasta() {
        let mut l = Listings::default();
        assert!(l.start("A"));
        assert!(!l.start("A"));
        assert!(l.finish("A", Err("rede".into())));
        assert!(matches!(l.get("A"), Some(Listing::Failed(_))));
    }

    #[test]
    fn titulo_mantem_subpastas() {
        let mut l = Listings::default();
        l.start("Animes/F/Frieren");
        l.start("Animes/F/Frieren/1080p");
        l.start("Animes/F/Frierenzinho");
        l.retain(Some("Animes/F/Frieren"));
        assert!(l.get("Animes/F/Frieren/1080p").is_some());
        assert!(l.get("Animes/F/Frierenzinho").is_none());
        l.retain(None);
        assert!(l.get("Animes/F/Frieren").is_none(), "início não guarda listagens");
    }
}
