//! Componentes do grupo `overlay`: Dialog, Tabs, Select, Tooltip e Toast.
//!
//! Todos seguem o mesmo padrão: `Comp::new(id, ...)` + métodos encadeados +
//! `.render(cx)` devolvendo `Element<V>`. O estado (aberto, seleção, fila) mora
//! na view do usuário e é alterado por callbacks ou por uma lente `|v| &mut v.campo`.
mod common;
mod dialog;
mod select;
mod select_state;
mod tabs;
mod toast;
mod toaster;
mod tooltip;

#[cfg(test)]
mod tests;

pub use dialog::Dialog;
pub use select::Select;
pub use select_state::SelectState;
pub use tabs::Tabs;
pub use toast::{Toast, ToastId, Toasts};
pub use toaster::Toaster;
pub use tooltip::{Tooltip, TooltipState};
