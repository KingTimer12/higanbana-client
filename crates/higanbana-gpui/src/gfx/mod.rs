//! Camada segura sobre o core C: contexto, janela, eventos, fontes, imagens e draw list.
pub mod backend;
pub mod clipboard;
pub mod context;
pub mod cursor;
pub mod decor;
pub mod draw;
pub mod event;
pub mod font;
pub mod image;
mod text_hit;
pub mod waker;
pub mod window;

pub use backend::{enumerate, BackendInfo, BackendKind};
pub use context::{Context, Stats};
pub use cursor::CursorStyle;
pub use decor::{Decorations, HitKind, HitRegion, WindowCommand, WindowState};
pub use draw::{ClipGuard, DrawList, Quad};
pub use event::{Event, Modifiers, MouseButton, NavDirection};
pub use font::{font_files_in, Font, FontSpec, TextMetrics};
pub use image::Image;
pub use waker::Waker;
pub use window::{Window, WindowOptions};
