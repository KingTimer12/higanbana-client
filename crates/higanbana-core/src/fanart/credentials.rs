//! Chave do fanart.tv embutida no build (mascarada com XOR, como as do TMDB).

use crate::secret::unmask;

/// Chave de API (`api-key`), se embutida no build.
pub fn api_key() -> Option<String> {
    unmask(env!("FANART_TV_MASKED"), env!("FANART_TV_MASK"))
}
