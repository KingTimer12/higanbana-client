//! higanbana-gpui — API Rust segura sobre o core C (`higanbana-gpui-sys`).
//!
//! Camadas: `gfx` (handles do core), `style`/`element` (descrição da UI),
//! `layout` (taffy), `paint` (draw list), `input` (hit test, hover, clique, rolagem)
//! `app` (laço que só refaz trabalho quando algo mudou), `anim` (tweens e timers),
//! `text_input` (edição de texto) e overlays (camada acima da árvore).
pub mod anim;
pub mod app;
pub mod color;
pub mod components;
pub mod cx;
pub mod element;
pub mod error;
pub mod geometry;
pub mod gfx;
pub mod input;
pub mod layout;
pub mod paint;
pub mod style;
pub mod text_input;
pub mod theme;
pub mod tree;
#[cfg(feature = "video")]
pub mod video;

pub use app::{App, View};
pub use color::{premultiply_rgba8, rgb, rgba, Color};
pub use anim::{Easing, Transition};
pub use cx::{EventCx, FocusInfo, RenderCx, TimerId};
pub use element::{
    div, img, overlay, scroll, text, text_field, Anchor, AnchorAlign, Element, ElementId, ImageData, ImageSource,
    MouseEvent, Overlay, PointerEvent, Side,
};
pub use input::{Key, KeyEvent};
pub use text_input::{InputOutcome, TextInputState};
pub use error::{Error, ErrorKind, Result};
pub use geometry::{point, size, Corners, Edges, Point, Rect, Size};
pub use gfx::*;
pub use style::{auto, pct, px, Length, Style, Styled, TextStyle};
pub use theme::Theme;
pub use tree::Tree;

/// Tudo que uma view costuma precisar.
pub mod prelude {
    pub use crate::{
        auto, div, img, overlay, pct, px, rgb, rgba, scroll, text, text_field, Anchor, AnchorAlign, App, BackendKind,
        Color, CursorStyle, Easing, Element, EventCx, ImageData, ImageSource, Key, KeyEvent, MouseEvent, PointerEvent,
        RenderCx, Side, Styled, TextInputState, Theme, Transition, View, WindowOptions, Decorations, HitKind,
        WindowCommand, WindowState,
    };
}
