//! Animação: tweens por id consultados no render (`cx.animate`), curvas de suavização e molas.
mod animator;
mod easing;
mod repeat;
mod spring;

pub use animator::Animator;
pub use easing::{Easing, Transition};
pub use repeat::Repeat;
pub use spring::Spring;
