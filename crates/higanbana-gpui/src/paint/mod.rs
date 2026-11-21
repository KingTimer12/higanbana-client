//! Pintura: árvore posicionada -> primitivas (quads, imagens, texto, clips, camadas).
mod canvas;
mod field;
pub mod cache;
mod gpu;
pub(crate) mod gradient;
mod images;
mod painter;
pub(crate) mod shape;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_state;
#[cfg(test)]
mod tests_fit;
#[cfg(test)]
mod tests_gradient;
#[cfg(test)]
mod tests_shape;

pub use canvas::Canvas;
pub use gpu::GpuCanvas;
pub use images::ImageCache;
