//! Estado da paleta `Command`: texto da busca, item selecionado e "buscando".
use crate::text_input::TextInputState;

/// Mora na view. `(grupo, item)` selecionado; `None` = o primeiro.
#[derive(Debug, Default)]
pub struct CommandState {
    pub input: TextInputState,
    pub(super) selected: Option<(usize, usize)>,
    pub loading: bool,
}

impl CommandState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(&self) -> &str {
        self.input.text()
    }

    pub fn selected(&self) -> Option<(usize, usize)> {
        self.selected
    }

    pub fn set_loading(&mut self, on: bool) {
        self.loading = on;
    }

    /// Volta a seleção ao primeiro item (resultados novos).
    pub fn reset_selection(&mut self) {
        self.selected = None;
    }

    /// Limpa busca e seleção (ao reabrir a paleta).
    pub fn clear(&mut self) {
        self.input.set_text("");
        self.selected = None;
        self.loading = false;
    }
}

/// Seleção efetiva: a guardada se existe, senão o primeiro item; `None` sem itens.
pub(super) fn clamp(sel: Option<(usize, usize)>, sizes: &[usize]) -> Option<(usize, usize)> {
    if let Some((g, i)) = sel
        && sizes.get(g).is_some_and(|&n| i < n) {
            return Some((g, i));
        }
    sizes.iter().position(|&n| n > 0).map(|g| (g, 0))
}

/// Próximo (`down`) ou anterior, atravessando grupos e parando nas pontas.
pub(super) fn step(cur: (usize, usize), sizes: &[usize], down: bool) -> (usize, usize) {
    let flat: Vec<(usize, usize)> = sizes.iter().enumerate().flat_map(|(g, &n)| (0..n).map(move |i| (g, i))).collect();
    let Some(pos) = flat.iter().position(|&p| p == cur) else { return cur };
    let next = if down { (pos + 1).min(flat.len() - 1) } else { pos.saturating_sub(1) };
    flat[next]
}
