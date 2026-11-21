//! Navegação: Sidebar (grupos, itens, recolher para ícones).
mod breadcrumb;
mod sidebar;
mod sidebar_item;
#[cfg(test)]
mod tests;

pub use breadcrumb::Breadcrumb;
pub use sidebar::{Sidebar, SidebarGroup, SidebarState, SidebarToggle, SIDEBAR_COLLAPSED_WIDTH, SIDEBAR_WIDTH};
pub use sidebar_item::SidebarItem;
