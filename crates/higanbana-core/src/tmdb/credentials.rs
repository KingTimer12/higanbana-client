//! Credenciais do TMDB embutidas no build (mascaradas com XOR, como a chave anon).

use crate::secret::unmask;

/// Token de leitura (Bearer), se embutido no build.
pub fn token() -> Option<String> {
    unmask(env!("TMDB_TOKEN_MASKED"), env!("TMDB_TOKEN_MASK"))
}

/// Chave de API v3 (query `api_key=`), se embutida no build.
pub fn api_key() -> Option<String> {
    unmask(env!("TMDB_KEY_MASKED"), env!("TMDB_KEY_MASK"))
}
