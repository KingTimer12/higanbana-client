//! Animação: tweens por id consultados no render (`cx.animate`) e curvas de suavização.
mod animator;
mod easing;

pub use animator::Animator;
pub use easing::{Easing, Transition};
