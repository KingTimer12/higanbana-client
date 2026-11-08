//! API da nuvem (`https://nuvem.anitsu.moe/api`), onde ficam os animes.
//!
//! Endpoints (descobertos no bundle JS da nuvem; todos exigem o cookie de sessão):
//! - `GET /files?path=<dir>`    → listagem de diretório
//! - `GET /search?q=<texto>`    → `{ query, results: [{ name, path, kind }] }`
//! - `GET /preview?path=<file>` → conteúdo para preview (imagens, vídeo, pdf...)
//! - `GET /download?path=<file>`→ arquivo inteiro (stream)

mod api;
pub(crate) mod gate;

pub use api::is_hidden;
pub mod download;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified: String,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub files: Vec<FileEntry>,
}

/// Categoria do resultado de busca (mesma lista do frontend da nuvem + `folder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    Anime,
    Desenho,
    Dorama,
    Manga,
    Ost,
    /// Pasta genérica (ex.: `Letra G`, `[#Temp]/...`).
    Folder,
    /// Categoria nova que o servidor passou a mandar.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    /// Caminho do diretório; serve direto para `nuvem_list`.
    pub path: String,
    pub kind: SearchKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_response() {
        let json = r#"{"query":"g","results":[
            {"name":"G-9","path":"Animes/Letra G/G-9","kind":"anime"},
            {"name":"Gantz G (2015)","path":"Mangás/Séries Mangás/Letra G/Gantz G (2015)","kind":"manga"},
            {"name":"Letra G","path":"Animes/Letra G","kind":"folder"},
            {"name":"X","path":"Y/X","kind":"novo_tipo"}
        ]}"#;
        let r: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.results.len(), 4);
        assert_eq!(r.results[0].kind, SearchKind::Anime);
        assert_eq!(r.results[1].kind, SearchKind::Manga);
        assert_eq!(r.results[2].kind, SearchKind::Folder);
        assert_eq!(r.results[3].kind, SearchKind::Other);
    }
}
