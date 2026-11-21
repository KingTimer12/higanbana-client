//! Componentes prontos no estilo shadcn/ui, montados sobre `element` + `theme`.
//!
//! - `basic`: Button, Badge, Card, Separator, Skeleton, Avatar.
//! - `form`: Input, Checkbox, Switch, Slider.
//! - `overlay`: Dialog, Tabs, Select, Tooltip, Toast.
//! - `window`: TitleBar (barra customizada).
pub mod basic;
pub mod form;
pub mod overlay;
pub mod window;
mod shared;

pub use basic::*;
pub use form::*;
pub use overlay::*;
pub use window::*;
pub use shared::{ComponentSize, Variant};
