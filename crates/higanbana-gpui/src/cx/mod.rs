//! Contextos entregues à view: de render (leitura, animação) e de evento (pedidos ao app).
mod event;
mod render;
mod timer;
mod task;
#[cfg(test)]
mod tests_task;

pub(crate) use event::FocusRequest;
pub use event::{EventCx, ScrollRequest};
pub use render::{FocusInfo, RenderCx};
pub use task::{TaskId, Tasks};
pub use timer::{TimerId, Timers};
