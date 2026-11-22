//! Leitor de mangá: estado e navegação puros (`state`, `nav`), carga (`load`) e tela.

pub mod load;
pub mod nav;
pub mod state;

use std::sync::Arc;

use higanbana_core::nuvem::FileEntry;

pub use state::ReaderUi;

/// O que está aberto no leitor. `dir` é a pasta dos arquivos; `title_path`, a chave das preferências.
#[derive(Debug, Clone)]
pub struct ReadTarget {
    pub dir: String,
    pub file: FileEntry,
    pub files: Arc<[FileEntry]>,
    pub title_path: String,
    pub title: String,
}

#[cfg(test)]
mod tests_nav;
#[cfg(test)]
mod tests_load;
