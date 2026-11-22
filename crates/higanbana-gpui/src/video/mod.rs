//! Player de vídeo com libmpv carregada em runtime (`feature = "video"`).
//!
//! No backend D3D11 o mpv desenha com OpenGL direto numa textura D3D11 que a UI mostra
//! (`gpu`). Nos outros backends, ou se a interop falhar, desenha na CPU (render API "sw")
//! e o quadro vira uma imagem dinâmica.
mod api;
mod gpu;
mod player;
mod render;
mod render_gl;
mod state;

pub use gpu::RenderPath;
pub use player::Player;
mod options;
pub use options::{PlayerOptions, DEFAULT_OPTIONS};
pub use state::{Chapter, PlayerState, Status, Track};

#[cfg(test)]
mod tests_options;
