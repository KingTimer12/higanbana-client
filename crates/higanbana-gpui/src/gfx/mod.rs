//! Camada segura sobre o core C: contexto, janela, eventos, fontes, imagens e draw list.
pub mod backend;
pub mod context;
pub mod draw;
pub mod event;
pub mod font;
pub mod image;
pub mod window;

pub use backend::{enumerate, BackendInfo, BackendKind};
pub use context::{Context, Stats};
pub use draw::{ClipGuard, DrawList, Quad};
pub use event::{Event, Modifiers, MouseButton};
pub use font::{Font, FontSpec, TextMetrics};
pub use image::Image;
pub use window::{Window, WindowOptions};
