//! higanbana-gpui — API Rust segura sobre o core C (`higanbana-gpui-sys`).
//!
//! Camadas: `gfx` (handles do core), `style`/`element` (descrição da UI),
//! `layout` (taffy), `paint` (draw list), `input` (hit test, hover, clique, rolagem)
//! e `app` (laço que só refaz trabalho quando algo mudou).
pub mod app;
pub mod color;
pub mod cx;
pub mod element;
pub mod error;
pub mod geometry;
pub mod gfx;
pub mod input;
pub mod layout;
pub mod paint;
pub mod style;
pub mod theme;
pub mod tree;

pub use app::{App, View};
pub use color::{premultiply_rgba8, rgb, rgba, Color};
pub use cx::{EventCx, RenderCx};
pub use element::{div, img, scroll, text, Element, ElementId, ImageData, ImageSource, MouseEvent};
pub use error::{Error, ErrorKind, Result};
pub use geometry::{point, size, Corners, Edges, Point, Rect, Size};
pub use gfx::*;
pub use style::{auto, pct, px, Length, Style, Styled, TextStyle};
pub use theme::Theme;
pub use tree::Tree;

/// Tudo que uma view costuma precisar.
pub mod prelude {
    pub use crate::{
        auto, div, img, pct, px, rgb, rgba, scroll, text, App, BackendKind, Color, Element, EventCx, ImageData,
        ImageSource, MouseEvent, RenderCx, Styled, Theme, View, WindowOptions,
    };
}
