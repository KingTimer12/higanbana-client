//! Credenciais do TMDB embutidas no build (mascaradas com XOR, como a chave anon).

/// Token de leitura (Bearer), se embutido no build.
pub fn token() -> Option<String> {
    unmask(env!("TMDB_TOKEN_MASKED"), env!("TMDB_TOKEN_MASK"))
}

/// Chave de API v3 (query `api_key=`), se embutida no build.
pub fn api_key() -> Option<String> {
    unmask(env!("TMDB_KEY_MASKED"), env!("TMDB_KEY_MASK"))
}

/// Desfaz a máscara XOR. `None` se vazio, de tamanhos diferentes ou inválido.
fn unmask(masked_hex: &str, mask_hex: &str) -> Option<String> {
    let masked = unhex(masked_hex)?;
    let mask = unhex(mask_hex)?;
    if masked.is_empty() || masked.len() != mask.len() {
        return None;
    }
    let bytes: Vec<u8> = masked.iter().zip(&mask).map(|(b, m)| b ^ m).collect();
    String::from_utf8(bytes).ok()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.is_ascii() || s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn empty_is_none() {
        assert_eq!(unmask("", ""), None);
    }

    #[test]
    fn roundtrip() {
        let masked: Vec<u8> = b"abc".iter().zip([1u8, 2, 3]).map(|(b, m)| b ^ m).collect();
        assert_eq!(unmask(&hex(&masked), &hex(&[1, 2, 3])), Some("abc".to_owned()));
    }

    #[test]
    fn different_lengths_is_none() {
        assert_eq!(unmask("6162", "010203"), None);
    }

    #[test]
    fn invalid_hex_is_none() {
        assert_eq!(unmask("zz", "01"), None);
    }
}
