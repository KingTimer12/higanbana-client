//! O que está tocando.

use higanbana_core::nuvem::FileEntry;

/// `dir` é a pasta do release (chave das faixas lembradas).
#[derive(Debug, Clone)]
pub struct PlayTarget {
    pub dir: String,
    pub file: FileEntry,
    pub files: Vec<FileEntry>,
    pub mal_id: Option<u32>,
    /// Seção `Animes/` (consulta o AniSkip); doramas e desenhos não.
    pub is_anime: bool,
    pub anime_title: String,
}
