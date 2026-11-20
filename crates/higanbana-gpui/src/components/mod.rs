//! Componentes prontos no estilo shadcn/ui, montados sobre `element` + `theme`.
//!
//! - `basic`: Button, Badge, Card, Separator, Skeleton, Avatar.
//! - `form`: Input, Checkbox, Switch, Slider.
//! - `overlay`: Dialog, Tabs, Select, Tooltip, Toast.
pub mod basic;
pub mod form;
pub mod overlay;
mod shared;

pub use basic::*;
pub use form::*;
pub use overlay::*;
pub use shared::{ComponentSize, Variant};
