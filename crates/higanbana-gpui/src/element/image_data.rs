//! Pixels de imagem pertencentes à view, enviados à GPU sob demanda.
use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;

use crate::gfx::Context;
use higanbana_gpui_sys as sys;

/// Imagem cuja textura vem de fora (alvo de vídeo na GPU): o cache pergunta a cada frame.
pub(crate) trait ExternalImage {
    /// Thread da UI, no `prepare`. `None` = nada para mostrar ainda.
    fn image(&self, ctx: &Context) -> Option<NonNull<sys::hg_image>>;
}
use std::sync::atomic::{AtomicU64, Ordering};

/// Imagem da view. Os pixels saem da RAM do Rust assim que vão para o core
/// (o core guarda a própria cópia para a troca de backend).
pub struct ImageData {
    id: u64,
    size: Cell<(u32, u32)>,
    pixels: RefCell<Option<Vec<u8>>>,
    /// Quadros trocados com `set_frame` (vídeo): pixels já pré-multiplicados.
    dynamic: bool,
    /// Recebe o buffer de volta depois do envio (vídeo reaproveita sem alocar).
    recycler: RefCell<Option<Box<dyn Fn(Vec<u8>)>>>,
    /// Textura de fora (vídeo na GPU); sem pixels.
    external: Option<Rc<dyn ExternalImage>>,
}

/// Referência compartilhada e barata de clonar para usar em `img()`.
pub type ImageSource = Rc<ImageData>;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl ImageData {
    /// RGBA8 com alfa reto, `width * height * 4` bytes.
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> ImageSource {
        assert_eq!(pixels.len(), width as usize * height as usize * 4, "tamanho do buffer RGBA8");
        Rc::new(ImageData {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            size: Cell::new((width, height)),
            pixels: RefCell::new(Some(pixels)),
            dynamic: false,
            recycler: RefCell::new(None),
            external: None,
        })
    }

    /// Imagem cujos pixels mudam (quadros de vídeo). Começa sem pixels: nada é desenhado
    /// até o primeiro `set_frame`.
    pub fn dynamic(width: u32, height: u32) -> ImageSource {
        Rc::new(ImageData {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            size: Cell::new((width, height)),
            pixels: RefCell::new(None),
            dynamic: true,
            recycler: RefCell::new(None),
            external: None,
        })
    }

    #[cfg(feature = "video")]
    /// Imagem desenhada direto da textura de `source` (o tamanho é só para o layout).
    pub(crate) fn external(width: u32, height: u32, source: Rc<dyn ExternalImage>) -> ImageSource {
        Rc::new(ImageData {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            size: Cell::new((width, height)),
            pixels: RefCell::new(None),
            dynamic: true,
            recycler: RefCell::new(None),
            external: Some(source),
        })
    }

    pub(crate) fn external_source(&self) -> Option<&Rc<dyn ExternalImage>> {
        self.external.as_ref()
    }

    #[cfg(feature = "video")]
    /// Tamanho para o layout (imagem externa acompanha o vídeo).
    pub(crate) fn set_size(&self, width: u32, height: u32) {
        self.size.set((width, height));
    }

    /// Troca o quadro (RGBA8 pré-multiplicado; opaco serve como está). O cache atualiza
    /// a textura no lugar se o tamanho não mudou, ou recria.
    pub fn set_frame(&self, width: u32, height: u32, pixels: Vec<u8>) {
        assert_eq!(pixels.len(), width as usize * height as usize * 4, "tamanho do quadro RGBA8");
        self.size.set((width, height));
        *self.pixels.borrow_mut() = Some(pixels);
    }

    /// Quem recebe o buffer depois que o quadro foi para a GPU (`recycle`).
    pub fn set_recycler(&self, f: impl Fn(Vec<u8>) + 'static) {
        *self.recycler.borrow_mut() = Some(Box::new(f));
    }

    /// Devolve o buffer já enviado ao reciclador (ou o libera).
    pub(crate) fn recycle(&self, pixels: Vec<u8>) {
        if let Some(f) = self.recycler.borrow().as_ref() {
            f(pixels);
        }
    }

    /// Pixel RGBA de um quadro ainda não enviado à GPU (testes/diagnóstico).
    pub fn pending_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let (w, h) = self.size.get();
        if x >= w || y >= h {
            return None;
        }
        let px = self.pixels.borrow();
        let i = (y as usize * w as usize + x as usize) * 4;
        px.as_ref().map(|p| [p[i], p[i + 1], p[i + 2], p[i + 3]])
    }

    pub fn is_dynamic(&self) -> bool {
        self.dynamic
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn size(&self) -> (u32, u32) {
        self.size.get()
    }

    /// Entrega os pixels para envio; depois disso a cópia Rust não existe mais.
    pub(crate) fn take_pixels(&self) -> Option<Vec<u8>> {
        self.pixels.borrow_mut().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reciclador_recebe_o_buffer_depois_do_envio() {
        let img = ImageData::dynamic(2, 2);
        let pool: Rc<RefCell<Vec<Vec<u8>>>> = Rc::default();
        let p = pool.clone();
        img.set_recycler(move |v| p.borrow_mut().push(v));
        img.set_frame(2, 2, Vec::with_capacity(64).into_iter().chain([0u8; 16]).collect());
        let px = img.take_pixels().unwrap();
        let ptr = px.as_ptr();
        img.recycle(px);
        assert_eq!(pool.borrow().len(), 1);
        assert_eq!(pool.borrow()[0].as_ptr(), ptr, "mesmo buffer, sem cópia");
    }

    #[test]
    fn sem_reciclador_o_buffer_e_liberado() {
        let img = ImageData::dynamic(1, 1);
        img.set_frame(1, 1, vec![0; 4]);
        let px = img.take_pixels().unwrap();
        img.recycle(px);
    }
}
