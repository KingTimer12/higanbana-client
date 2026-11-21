//! Animação: tweens por id consultados no render (`cx.animate`), curvas de suavização e molas.
mod animator;
mod easing;
mod spring;

pub use animator::Animator;
pub use easing::{Easing, Transition};
pub use spring::Spring;
