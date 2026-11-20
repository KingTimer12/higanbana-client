//! Entrada: hit testing, hover, clique e rolagem sobre a `Tree`.
mod dispatch;
mod hit;

#[cfg(test)]
mod tests;

pub use dispatch::SCROLL_STEP;
