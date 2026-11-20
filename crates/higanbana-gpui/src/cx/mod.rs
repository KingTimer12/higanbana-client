//! Contextos entregues à view: de render (leitura, animação) e de evento (pedidos ao app).
mod event;
mod render;
mod timer;

pub(crate) use event::FocusRequest;
pub use event::EventCx;
pub use render::{FocusInfo, RenderCx};
pub use timer::{TimerId, Timers};
