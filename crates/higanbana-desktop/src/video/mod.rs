//! Player de vídeo: o mpv desenha pela `higanbana-gpui` (render API sw) e a tela
//! monta os controles da master com as peças da lib.

pub(crate) mod convert;
mod options;
mod session;
mod target;
mod thumbs;
mod view;

// Usados pela tela (Tasks 7–10 do Plano 3).
#[allow(unused_imports)]
pub use options::{BACK_CACHES, CACHES};
#[allow(unused_imports)]
pub use session::{Action, PlayerSession};
#[allow(unused_imports)]
pub use target::PlayTarget;

pub use view::{view, PlayerMsg, PlayerView, Spare};

#[cfg(test)]
mod tests_session;
