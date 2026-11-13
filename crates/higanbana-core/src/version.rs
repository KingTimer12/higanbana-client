//! Versão do aplicativo, lida do arquivo `VERSION` na raiz do repositório.
//!
//! Não vem do `Cargo.toml` de propósito: mexer na versão do manifesto invalida o
//! cache de build do CI (e de quem compila local). O `VERSION` só recompila este crate.

const RAW: &str = include_str!("../../../VERSION");

/// Versão atual, sem o `v` e sem quebra de linha (`0.1.0`).
pub fn current() -> &'static str {
    RAW.trim().trim_start_matches('v')
}

/// Divide `1.2.3` (aceita prefixo `v` e sufixo `-beta`) em números; `None` se inválido.
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next()?.parse().ok()?;
    it.next().is_none().then_some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_is_valid() {
        assert!(parse(current()).is_some());
    }

    #[test]
    fn parses_and_orders() {
        assert_eq!(parse("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse("0.10.0-beta.1"), Some((0, 10, 0)));
        assert_eq!(parse("1.2"), None);
        assert!(parse("0.10.0") > parse("0.9.9"));
    }
}
