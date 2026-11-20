//! Estado do Select guardado na view e a lógica de teclado (testável sem árvore).
use crate::input::Key;

/// Estado de um [`Select`](super::Select): aberto, item escolhido e item destacado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectState {
    pub open: bool,
    pub selected: Option<usize>,
    /// Item sob o mouse/teclado enquanto aberto.
    pub highlighted: usize,
}

/// O que uma tecla fez com o estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyOutcome {
    Ignored,
    Changed,
    Chose(usize),
}

impl SelectState {
    pub fn with_selected(selected: Option<usize>) -> Self {
        Self { selected, ..Self::default() }
    }

    /// Abre destacando o escolhido (ou o primeiro).
    pub fn open(&mut self) {
        self.open = true;
        self.highlighted = self.selected.unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle(&mut self) {
        if self.open { self.close() } else { self.open() }
    }

    /// Escolhe `i` e fecha.
    pub fn choose(&mut self, i: usize) {
        self.selected = Some(i);
        self.highlighted = i;
        self.open = false;
    }

    /// Teclado no gatilho com `n` itens.
    pub(super) fn key(&mut self, key: Key, n: usize) -> KeyOutcome {
        if n == 0 {
            return KeyOutcome::Ignored;
        }
        if !self.open {
            return match key {
                Key::Down | Key::Up | Key::Enter | Key::Space => {
                    self.open();
                    KeyOutcome::Changed
                }
                _ => KeyOutcome::Ignored,
            };
        }
        match key {
            Key::Down => self.highlighted = (self.highlighted + 1).min(n - 1),
            Key::Up => self.highlighted = self.highlighted.saturating_sub(1),
            Key::Home => self.highlighted = 0,
            Key::End => self.highlighted = n - 1,
            Key::Enter | Key::Space => {
                let i = self.highlighted.min(n - 1);
                self.choose(i);
                return KeyOutcome::Chose(i);
            }
            Key::Escape | Key::Tab => self.close(),
            _ => return KeyOutcome::Ignored,
        }
        KeyOutcome::Changed
    }

    /// Type-ahead: destaca o próximo item que começa com `c` (sem diferenciar caixa).
    pub(super) fn type_ahead<'a>(&mut self, c: char, labels: impl Iterator<Item = &'a str>) -> bool {
        let labels: Vec<&str> = labels.collect();
        let n = labels.len();
        let c = c.to_lowercase().next().unwrap_or(c);
        let start = if self.open { self.highlighted + 1 } else { 0 };
        let hit = (0..n).map(|k| (start + k) % n.max(1)).find(|&i| labels[i].chars().next().and_then(|f| f.to_lowercase().next()) == Some(c));
        if let Some(i) = hit {
            self.highlighted = i;
            if !self.open {
                self.open = true;
            }
        }
        hit.is_some()
    }
}
