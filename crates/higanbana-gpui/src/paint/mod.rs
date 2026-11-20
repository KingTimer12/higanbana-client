//! Pintura: árvore posicionada -> primitivas (quads, imagens, texto, clips, camadas).
mod canvas;
mod gpu;
mod images;
mod painter;

#[cfg(test)]
mod tests;

pub use canvas::Canvas;
pub use gpu::GpuCanvas;
pub use images::ImageCache;
