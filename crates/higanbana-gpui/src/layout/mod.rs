//! Layout com taffy: estilo -> taffy, medição de texto e posições absolutas.
mod engine;
mod measure;
mod position;
mod style_map;

#[cfg(test)]
pub(crate) mod tests;

pub use engine::LayoutEngine;
pub use measure::{MeasureCache, TextMeasurer};
pub(crate) use position::{padding_box, position, ScrollOffsets};

#[cfg(test)]
pub(crate) use measure::tests::FakeMeasurer;
