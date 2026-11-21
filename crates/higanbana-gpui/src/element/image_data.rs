//! Pixels de imagem pertencentes à view, enviados à GPU sob demanda.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Imagem da view. Os pixels saem da RAM do Rust assim que vão para o core
/// (o core guarda a própria cópia para a troca de backend).
pub struct ImageData {
    id: u64,
    size: Cell<(u32, u32)>,
    pixels: RefCell<Option<Vec<u8>>>,
    /// Quadros trocados com `set_frame` (vídeo): pixels já pré-multiplicados.
    dynamic: bool,
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
        })
    }

    /// Troca o quadro (RGBA8 pré-multiplicado; opaco serve como está). O cache atualiza
    /// a textura no lugar se o tamanho não mudou, ou recria.
    pub fn set_frame(&self, width: u32, height: u32, pixels: Vec<u8>) {
        assert_eq!(pixels.len(), width as usize * height as usize * 4, "tamanho do quadro RGBA8");
        self.size.set((width, height));
        *self.pixels.borrow_mut() = Some(pixels);
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
