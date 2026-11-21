//! Lista virtual: só constrói os itens que aparecem (mais uma tela de folga de
//! cada lado). Rolar uma lista virtual reconstrói a view (os itens mudam).
mod virtual_list;
#[cfg(test)]
mod tests;

pub use virtual_list::{item_offset, visible_range, Range, VirtualList};
