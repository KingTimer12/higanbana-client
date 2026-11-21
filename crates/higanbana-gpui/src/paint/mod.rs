//! Pintura: árvore posicionada -> primitivas (quads, imagens, texto, clips, camadas).
mod canvas;
mod field;
mod gpu;
pub(crate) mod gradient;
mod images;
mod painter;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_state;
#[cfg(test)]
mod tests_fit;
#[cfg(test)]
mod tests_gradient;

pub use canvas::Canvas;
pub use gpu::GpuCanvas;
pub use images::ImageCache;
