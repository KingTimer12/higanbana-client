//! Animação: tweens por id consultados no render (`cx.animate`), curvas, molas e presença.
mod animator;
mod easing;
mod lerp;
mod presence;
mod repeat;
mod spring;

pub use animator::Animator;
pub use easing::{Easing, Transition};
pub use lerp::Animatable;
pub use repeat::Repeat;
pub use spring::Spring;
