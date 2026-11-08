//! Player de vídeo: mpv numa janela filha, controles do GPUI por cima.

mod mpv;
mod surface;
mod thumbs;
mod view;

pub use surface::Surface;
pub use view::{PlayTarget, PlayerEvent, PlayerView};
