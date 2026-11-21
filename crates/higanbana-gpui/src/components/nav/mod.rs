//! Navegação: Sidebar (grupos, itens, recolher para ícones).
mod sidebar;
mod sidebar_item;
#[cfg(test)]
mod tests;

pub use sidebar::{Sidebar, SidebarGroup, SidebarState, SidebarToggle, SIDEBAR_COLLAPSED_WIDTH, SIDEBAR_WIDTH};
pub use sidebar_item::SidebarItem;
