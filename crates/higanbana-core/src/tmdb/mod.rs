//! TMDB (The Movie Database): miniaturas, títulos e notas dos episódios. Só leitura.
//!
//! Opcional: sem `TMDB_TOKEN`/`TMDB_KEY` no build o cliente responde `NotConfigured`
//! sem tocar na rede e o resto do app segue sem esses dados.

pub mod client;
pub mod credentials;
pub mod matching;
pub mod service;
pub mod types;

pub use service::{EpisodeView, Tmdb, WorkEpisodes, work_key};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TmdbError {
    #[error("TMDB não configurado")]
    NotConfigured,
    #[error("TMDB limitou as requisições")]
    RateLimited,
    #[error("não encontrado no TMDB")]
    NotFound,
    #[error("falha HTTP no TMDB: {0}")]
    Http(String),
    #[error("resposta inválida do TMDB: {0}")]
    Decode(String),
}

pub type TmdbResult<T> = Result<T, TmdbError>;

/// URL da imagem de um episódio (`path` vem com a barra inicial, ex. `/abc.jpg`).
pub fn still_url(path: &str, size: &str) -> String {
    format!("https://image.tmdb.org/t/p/{size}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn still_url_joins_size_and_path() {
        assert_eq!(still_url("/abc.jpg", "w300"), "https://image.tmdb.org/t/p/w300/abc.jpg");
    }
}
