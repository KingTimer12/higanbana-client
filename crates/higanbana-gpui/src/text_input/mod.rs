//! Primitivo de entrada de texto de uma linha: estado editável (`TextInputState`)
//! independente de GPU, usado pelo elemento `text_field`.
mod edit;
mod keys;
mod nav;
mod state;

#[cfg(test)]
mod tests;

pub use edit::{Clipboard, SystemClipboard};
pub use keys::InputOutcome;
pub use state::TextInputState;
