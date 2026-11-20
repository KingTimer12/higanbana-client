//! Máscara de senha: o campo pinta bolinhas e os índices de cursor/clique são
//! convertidos entre o texto real e o mascarado.
use crate::element::{Element, Kind};
use crate::text_input::TextInputState;
use std::borrow::Cow;

/// Caractere mostrado no lugar de cada caractere da senha.
pub(super) const BULLET: char = '\u{2022}';

/// Texto mascarado (uma bolinha por caractere).
pub(super) fn mask(text: &str) -> String {
    std::iter::repeat_n(BULLET, text.chars().count()).collect()
}

/// Byte no texto real -> byte no texto mascarado.
pub(super) fn masked_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count() * BULLET.len_utf8()
}

/// Byte no texto mascarado -> byte no texto real.
pub(super) fn unmask(text: &str, masked: usize) -> usize {
    let n = masked / BULLET.len_utf8();
    text.char_indices().nth(n).map_or(text.len(), |(i, _)| i)
}

/// Ajusta a cópia de pintura do campo: máscara (senha) e placeholder próprio.
pub(super) fn patch_field<V>(e: &mut Element<V>, state: &TextInputState, password: bool, placeholder: Option<Cow<'static, str>>) {
    let Kind::TextField(f) = &mut e.kind else { return };
    if let Some(p) = placeholder {
        f.placeholder = p;
    }
    if password {
        f.text = mask(&state.text);
        f.caret = masked_index(&state.text, state.caret);
        f.anchor = masked_index(&state.text, state.anchor);
    }
}
