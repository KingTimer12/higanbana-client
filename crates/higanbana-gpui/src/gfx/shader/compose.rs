//! Monta o módulo WGSL completo: prelúdio + código do usuário + pontos de entrada.

const PRELUDE: &str = include_str!("prelude.wgsl");
const ENTRY: &str = include_str!("entry.wgsl");

/// Módulo completo e quantas linhas o prelúdio ocupa antes do código do usuário
/// (linha do usuário = linha no módulo − esse número).
pub(crate) fn compose(user: &str) -> (String, u32) {
    debug_assert!(PRELUDE.ends_with('\n'));
    let mut full = String::with_capacity(PRELUDE.len() + user.len() + ENTRY.len() + 1);
    full.push_str(PRELUDE);
    full.push_str(user);
    if !user.ends_with('\n') {
        full.push('\n');
    }
    full.push_str(ENTRY);
    (full, PRELUDE.lines().count() as u32)
}
