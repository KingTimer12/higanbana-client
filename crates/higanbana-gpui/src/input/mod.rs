//! Entrada sobre a `Tree`: hit testing, hover, clique, ponteiro com captura, rolagem,
//! foco, teclado e fechamento de overlays.
mod dismiss;
mod dispatch;
mod focus;
mod hit;
mod hover;
mod key;
mod keyboard;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_focus;
#[cfg(test)]
mod tests_overlay;

pub use dispatch::SCROLL_STEP;
pub use key::{Key, KeyEvent};
