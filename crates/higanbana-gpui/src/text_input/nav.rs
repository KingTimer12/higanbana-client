//! Movimento do cursor: por grafema, por palavra e para as pontas.
use super::state::TextInputState;
use unicode_segmentation::UnicodeSegmentation;

/// Classe de caractere para pular palavras (Ctrl+setas).
#[derive(PartialEq, Eq, Clone, Copy)]
enum Class {
    Space,
    Word,
    Punct,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// Início do grafema anterior a `i`.
pub(crate) fn prev_grapheme(s: &str, i: usize) -> usize {
    s[..i].grapheme_indices(true).next_back().map_or(0, |(b, _)| b)
}

/// Fim do grafema que começa em `i`.
pub(crate) fn next_grapheme(s: &str, i: usize) -> usize {
    s[i..].graphemes(true).next().map_or(s.len(), |g| i + g.len())
}

/// Ctrl+Esquerda: pula espaços para trás e depois a palavra.
pub(crate) fn prev_word(s: &str, i: usize) -> usize {
    let mut it = s[..i].char_indices().rev().peekable();
    while it.next_if(|(_, c)| class(*c) == Class::Space).is_some() {}
    let Some(&(_, first)) = it.peek() else { return 0 };
    let k = class(first);
    let mut start = i;
    while let Some((b, _)) = it.next_if(|(_, c)| class(*c) == k) {
        start = b;
    }
    start.min(i)
}

/// Ctrl+Direita: pula a palavra atual e os espaços seguintes (início da próxima).
pub(crate) fn next_word(s: &str, i: usize) -> usize {
    let mut it = s[i..].char_indices().map(|(b, c)| (i + b, c)).peekable();
    if let Some(&(_, first)) = it.peek() {
        let k = class(first);
        if k != Class::Space {
            while it.next_if(|(_, c)| class(*c) == k).is_some() {}
        }
    }
    while it.next_if(|(_, c)| class(*c) == Class::Space).is_some() {}
    it.peek().map_or(s.len(), |(b, _)| *b)
}

impl TextInputState {
    /// Esquerda (Ctrl = palavra; Shift = estende a seleção). Sem Shift e com seleção,
    /// recolhe para o início dela.
    pub fn move_left(&mut self, word: bool, extend: bool) {
        let to = if !extend && self.has_selection() {
            self.selection().start
        } else if word {
            prev_word(&self.text, self.caret)
        } else {
            prev_grapheme(&self.text, self.caret)
        };
        self.set_caret(to, extend);
    }

    /// Direita (Ctrl = palavra; Shift = estende a seleção).
    pub fn move_right(&mut self, word: bool, extend: bool) {
        let to = if !extend && self.has_selection() {
            self.selection().end
        } else if word {
            next_word(&self.text, self.caret)
        } else {
            next_grapheme(&self.text, self.caret)
        };
        self.set_caret(to, extend);
    }

    /// Home.
    pub fn move_home(&mut self, extend: bool) {
        self.set_caret(0, extend);
    }

    /// End.
    pub fn move_end(&mut self, extend: bool) {
        self.set_caret(self.text.len(), extend);
    }

    /// Seleciona a palavra sob `index` (duplo clique).
    pub fn select_word_at(&mut self, index: usize) {
        let mut i = index.min(self.text.len());
        while !self.text.is_char_boundary(i) {
            i -= 1;
        }
        let start = if i < self.text.len() { prev_word(&self.text, next_grapheme(&self.text, i)) } else { prev_word(&self.text, i) };
        let mut end = start;
        if let Some(c) = self.text[start..].chars().next() {
            let k = class(c);
            end = self.text[start..].char_indices().find(|(_, c)| class(*c) != k).map_or(self.text.len(), |(b, _)| start + b);
        }
        self.anchor = start;
        self.caret = end;
        self.touch();
    }
}
