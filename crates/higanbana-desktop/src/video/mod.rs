//! Player de vídeo: o mpv desenha pela `higanbana-gpui` (render API sw) e a tela
//! monta os controles da master com as peças da lib.

mod convert;
mod options;
mod session;
mod target;

// Usados pela tela (Tasks 7–10 do Plano 3).
#[allow(unused_imports)]
pub use options::{BACK_CACHES, CACHES};
#[allow(unused_imports)]
pub use session::{Action, PlayerSession};
#[allow(unused_imports)]
pub use target::PlayTarget;

#[cfg(test)]
mod tests_session;
