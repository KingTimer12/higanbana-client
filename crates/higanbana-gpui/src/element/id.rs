//! Identidade estável de elementos, como hash de 64 bits (sem alocar string).

/// Identificador de elemento. `"lista"` ou `("item", 3)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(pub u64);

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

impl From<&str> for ElementId {
    fn from(s: &str) -> Self {
        ElementId(fnv(FNV_OFFSET, s.as_bytes()))
    }
}

impl From<(&str, usize)> for ElementId {
    fn from((s, i): (&str, usize)) -> Self {
        ElementId(fnv(fnv(FNV_OFFSET, s.as_bytes()), &(i as u64).to_le_bytes()))
    }
}

impl From<String> for ElementId {
    fn from(s: String) -> Self {
        ElementId::from(s.as_str())
    }
}

/// Prefixo + chave de texto (`("pagina", caminho)`), distinto de `"prefixo" + "chave"` colados.
impl From<(&str, &str)> for ElementId {
    fn from((a, b): (&str, &str)) -> Self {
        ElementId(fnv(fnv(fnv(FNV_OFFSET, a.as_bytes()), &[0xff]), b.as_bytes()))
    }
}

impl From<u64> for ElementId {
    fn from(v: u64) -> Self {
        ElementId(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_distinct() {
        assert_eq!(ElementId::from("a"), ElementId::from("a"));
        assert_ne!(ElementId::from("a"), ElementId::from("b"));
        assert_ne!(ElementId::from(("item", 1)), ElementId::from(("item", 2)));
        assert_eq!(ElementId::from(String::from("a")), ElementId::from("a"));
        assert_ne!(ElementId::from(("ab", "c")), ElementId::from(("a", "bc")));
        assert_eq!(ElementId::from(("p", "x/y")), ElementId::from(("p", "x/y")));
    }
}
