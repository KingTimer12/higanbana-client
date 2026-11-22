//! Contextos entregues à view: de render (leitura, animação) e de evento (pedidos ao app).
mod event;
mod render;
mod timer;
mod mailbox;
mod task;
#[cfg(test)]
mod tests_mailbox;
#[cfg(test)]
mod tests_task;

pub use mailbox::{Inbox, Mailbox};
pub(crate) use mailbox::inbox;

pub(crate) use event::FocusRequest;
pub use event::{EventCx, ScrollRequest};
pub use render::{FocusInfo, RenderCx};
pub use task::{TaskId, Tasks};
pub use timer::{TimerId, Timers};
