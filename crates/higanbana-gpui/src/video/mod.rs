//! Player de vídeo com libmpv carregada em runtime (`feature = "video"`).
//!
//! O mpv decodifica e desenha cada quadro na CPU (render API "sw"); o quadro vira uma
//! imagem dinâmica do higanbana, então funciona igual em todos os backends.
mod api;
mod player;
mod render;
mod state;

pub use player::Player;
pub use state::{PlayerState, Status, Track};
