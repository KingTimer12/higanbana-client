//! Player de vídeo com libmpv carregada em runtime (`feature = "video"`).
//!
//! O mpv decodifica e desenha cada quadro na CPU (render API "sw"); o quadro vira uma
//! imagem dinâmica do higanbana, então funciona igual em todos os backends.
mod api;
mod player;
mod render;
mod state;

pub use player::Player;
mod options;
pub use options::{PlayerOptions, DEFAULT_OPTIONS};
pub use state::{Chapter, PlayerState, Status, Track};

#[cfg(test)]
mod tests_options;
