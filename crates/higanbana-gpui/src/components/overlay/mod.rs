//! Componentes do grupo `overlay`: Dialog, Tabs, Select, Tooltip e Toast.
//!
//! Todos seguem o mesmo padrão: `Comp::new(id, ...)` + métodos encadeados +
//! `.render(cx)` devolvendo `Element<V>`. O estado (aberto, seleção, fila) mora
//! na view do usuário e é alterado por callbacks ou por uma lente `|v| &mut v.campo`.
mod command;
mod command_state;
mod common;
mod dialog;
mod dropdown_menu;
mod notifier;
mod popover;
mod select;
mod select_state;
mod tab_bar;
mod tabs;
mod toast;
mod toaster;
mod tooltip;

#[cfg(test)]
pub(crate) mod tests;

pub use command::{Command, CommandGroup, CommandItem};
pub use command_state::CommandState;
pub use dialog::Dialog;
pub use dropdown_menu::DropdownMenu;
pub use popover::{Popover, PopoverAnchor, PopoverState};
pub use select::Select;
pub use select_state::SelectState;
pub use tab_bar::TabBar;
pub use tabs::Tabs;
pub use notifier::Notifier;
pub use toast::{Toast, ToastId, ToastTone, Toasts, NOTIFICATION_DURATION};
pub use toaster::Toaster;
pub use tooltip::{Tooltip, TooltipState};
