//! Pintura: árvore posicionada -> primitivas (quads, imagens, texto, clips, camadas).
mod canvas;
mod field;
mod gpu;
mod images;
mod painter;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_state;

pub use canvas::Canvas;
pub use gpu::GpuCanvas;
pub use images::ImageCache;
