//! Destino de pintura em pixels lógicos e o controle de camadas.
use crate::color::Color;
use crate::element::ImageSource;
use crate::geometry::{Corners, Point, Rect};
use crate::gfx::Quad;
use crate::style::TextStyle;

/// Superfície de desenho. A implementação de GPU escala para pixels físicos;
/// testes usam uma que só grava as chamadas.
pub trait Canvas {
    fn quad(&mut self, q: &Quad);
    /// `tint` multiplica a imagem (opacidade no alfa; cor nos ícones).
    fn image(&mut self, src: &ImageSource, rect: Rect, radius: Corners<f32>, tint: Color);
    fn text(&mut self, text: &str, style: &TextStyle, origin: Point, max_width: Option<f32>, color: Color);
    fn push_clip(&mut self, r: Rect);
    fn pop_clip(&mut self);
    /// Tudo depois fica por cima de tudo antes.
    fn layer(&mut self);
}

/// Dentro de uma camada o core desenha quads, depois imagens, depois texto.
/// Este adaptador só abre camada nova quando um quad (ou imagem) cairia
/// por cima de texto/imagem já emitido na camada atual — o mínimo de camadas.
pub(crate) struct Layered<'a> {
    inner: &'a mut dyn Canvas,
    text: Option<Rect>,
    images: Option<Rect>,
    pub layers: u32,
}

fn grow(acc: &mut Option<Rect>, r: Rect) {
    *acc = Some(acc.map_or(r, |a| a.union(&r)));
}

fn hits(acc: Option<Rect>, r: &Rect) -> bool {
    acc.is_some_and(|a| a.intersects(r))
}

impl<'a> Layered<'a> {
    pub fn new(inner: &'a mut dyn Canvas) -> Self {
        Self { inner, text: None, images: None, layers: 0 }
    }

    pub fn new_layer(&mut self) {
        self.inner.layer();
        self.text = None;
        self.images = None;
        self.layers += 1;
    }

    pub fn quad(&mut self, q: &Quad) {
        let reach = q.rect.translate(-q.shadow_blur, -q.shadow_blur);
        let reach = Rect::new(
            reach.origin.x,
            reach.origin.y,
            q.rect.size.width + 2.0 * q.shadow_blur,
            q.rect.size.height + 2.0 * q.shadow_blur,
        );
        if hits(self.text, &reach) || hits(self.images, &reach) {
            self.new_layer();
        }
        self.inner.quad(q);
    }

    pub fn image(&mut self, src: &ImageSource, rect: Rect, radius: Corners<f32>, tint: Color) {
        if hits(self.text, &rect) {
            self.new_layer();
        }
        self.inner.image(src, rect, radius, tint);
        grow(&mut self.images, rect);
    }

    pub fn text(&mut self, text: &str, style: &TextStyle, bounds: Rect, max_width: Option<f32>, color: Color) {
        self.inner.text(text, style, bounds.origin, max_width, color);
        grow(&mut self.text, bounds);
    }

    /// No core, empilhar/desempilhar clip já abre um lote novo (camada).
    pub fn push_clip(&mut self, r: Rect) {
        self.inner.push_clip(r);
        self.text = None;
        self.images = None;
    }

    pub fn pop_clip(&mut self) {
        self.inner.pop_clip();
        self.text = None;
        self.images = None;
    }
}
