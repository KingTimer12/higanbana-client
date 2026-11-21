//! Estado de um campo de texto de uma linha: buffer UTF-8, cursor e seleção.
use std::borrow::Cow;
use std::ops::Range;
use std::time::Instant;

/// Estado editável guardado na view. Índices são bytes UTF-8 e sempre caem
/// em fronteira de caractere (nunca partem um codepoint).
#[derive(Debug, Clone)]
pub struct TextInputState {
    pub(crate) text: String,
    /// Posição do cursor (ponta móvel da seleção).
    pub(crate) caret: usize,
    /// Ponta fixa da seleção; == caret quando não há seleção.
    pub(crate) anchor: usize,
    pub(crate) placeholder: Cow<'static, str>,
    /// Última edição/movimento: o cursor fica aceso logo depois (piscar recomeça).
    pub(crate) epoch: Instant,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInputState {
    pub fn new() -> Self {
        Self { text: String::new(), caret: 0, anchor: 0, placeholder: Cow::Borrowed(""), epoch: Instant::now() }
    }

    /// Estado com texto inicial e cursor no fim.
    pub fn with_text(text: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.set_text(text);
        s
    }

    /// Texto mostrado em cinza enquanto o campo está vazio.
    pub fn placeholder(mut self, text: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn set_placeholder(&mut self, text: impl Into<Cow<'static, str>>) {
        self.placeholder = text.into();
    }

    pub fn placeholder_text(&self) -> &str {
        &self.placeholder
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Troca o texto todo (quebras de linha viram espaço) e põe o cursor no fim.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let mut t: String = text.into();
        if t.contains(['\n', '\r']) {
            t = t.replace("\r\n", " ").replace(['\n', '\r'], " ");
        }
        self.text = t;
        self.caret = self.text.len();
        self.anchor = self.caret;
        self.touch();
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Seleção ordenada (vazia se não há seleção).
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    /// Instante da última mudança (base do piscar do cursor).
    pub fn epoch(&self) -> Instant {
        self.epoch
    }

    /// Põe o cursor em `index` (ajustado para a fronteira de caractere anterior).
    /// `extend` mantém a âncora (Shift+clique, arraste).
    pub fn set_caret(&mut self, index: usize, extend: bool) {
        let mut i = index.min(self.text.len());
        while !self.text.is_char_boundary(i) {
            i -= 1;
        }
        self.caret = i;
        if !extend {
            self.anchor = i;
        }
        self.touch();
    }

    /// Seleciona tudo (Ctrl+A).
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
        self.touch();
    }

    pub(crate) fn touch(&mut self) {
        self.epoch = Instant::now();
    }
}
