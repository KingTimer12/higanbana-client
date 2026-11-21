//! Componentes prontos no estilo shadcn/ui, montados sobre `element` + `theme`.
//!
//! - `basic`: Button, Badge, Card, Separator, Skeleton, Avatar.
//! - `form`: Input, Checkbox, Switch, Slider.
//! - `overlay`: Dialog, Tabs, Select, Tooltip, Toast.
//! - `nav`: Sidebar.
//! - `window`: TitleBar (barra customizada).
//! - `video` (feature `video`): VideoPlayer.
pub mod basic;
pub mod form;
pub mod nav;
pub mod overlay;
#[cfg(feature = "video")]
pub mod video;
pub mod window;
mod shared;

pub use basic::*;
pub use form::*;
pub use nav::*;
pub use overlay::*;
#[cfg(feature = "video")]
pub use video::{AutoHide, ClickGate, VideoSurface, DOUBLE_CLICK, HIDE_AFTER, VideoPlayer, VideoState};
pub use window::*;
pub use shared::{ComponentSize, Variant};
