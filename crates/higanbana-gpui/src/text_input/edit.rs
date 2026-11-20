//! Edição: inserir, apagar e área de transferência.
use super::nav::{next_grapheme, next_word, prev_word};
use super::state::TextInputState;

/// Destino de copiar/colar. O app usa a área de transferência do sistema;
/// testes usam uma em memória.
pub trait Clipboard {
    fn set_text(&mut self, text: &str);
    fn get_text(&mut self) -> Option<String>;
}

/// Área de transferência do Windows.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn set_text(&mut self, text: &str) {
        if let Err(e) = crate::gfx::clipboard::set_text(text) {
            eprintln!("higanbana-gpui: copiar falhou: {e}");
        }
    }

    fn get_text(&mut self) -> Option<String> {
        crate::gfx::clipboard::get_text()
    }
}

impl TextInputState {
    /// Substitui a seleção (ou insere no cursor) por `s`. Quebras de linha viram espaço.
    pub fn insert(&mut self, s: &str) {
        let r = self.selection();
        if s.contains(['\n', '\r']) {
            let clean = s.replace("\r\n", " ").replace(['\n', '\r'], " ");
            self.text.replace_range(r.clone(), &clean);
            self.caret = r.start + clean.len();
        } else {
            self.text.replace_range(r.clone(), s);
            self.caret = r.start + s.len();
        }
        self.anchor = self.caret;
        self.touch();
    }

    /// Apaga a seleção; devolve `false` se não havia.
    fn delete_selection(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        self.insert("");
        true
    }

    /// Backspace (Ctrl = palavra). Apaga por codepoint para trás — como os editores
    /// do sistema, desfaz acentos combinantes um a um; nunca parte UTF-8.
    pub fn backspace(&mut self, word: bool) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.caret == 0 {
            return false;
        }
        let from = if word {
            prev_word(&self.text, self.caret)
        } else {
            self.text[..self.caret].char_indices().next_back().map_or(0, |(b, _)| b)
        };
        self.text.replace_range(from..self.caret, "");
        self.caret = from;
        self.anchor = from;
        self.touch();
        true
    }

    /// Delete (Ctrl = palavra), por grafema para frente.
    pub fn delete(&mut self, word: bool) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.caret >= self.text.len() {
            return false;
        }
        let to = if word { next_word(&self.text, self.caret) } else { next_grapheme(&self.text, self.caret) };
        self.text.replace_range(self.caret..to, "");
        self.anchor = self.caret;
        self.touch();
        true
    }

    /// Ctrl+C: copia a seleção. `false` se não havia seleção.
    pub fn copy(&self, clip: &mut dyn Clipboard) -> bool {
        if !self.has_selection() {
            return false;
        }
        clip.set_text(self.selected_text());
        true
    }

    /// Ctrl+X.
    pub fn cut(&mut self, clip: &mut dyn Clipboard) -> bool {
        self.copy(clip) && self.delete_selection()
    }

    /// Ctrl+V.
    pub fn paste(&mut self, clip: &mut dyn Clipboard) -> bool {
        match clip.get_text() {
            Some(t) if !t.is_empty() => {
                self.insert(&t);
                true
            }
            _ => false,
        }
    }
}
