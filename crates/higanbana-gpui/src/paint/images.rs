//! Cache de imagens na GPU, chaveado pelo id da `ImageData` da view.
use crate::element::ImageSource;
use crate::gfx::{Context, Image};
use crate::tree::node::NodeKind;
use crate::tree::Tree;
use std::collections::HashMap;
use std::rc::Rc;

/// Mantém uma `Image` por `ImageData` em uso. Quando só o cache segura a fonte
/// (a view e a árvore a soltaram), a textura é destruída.
pub struct ImageCache<'c> {
    ctx: &'c Context,
    map: HashMap<u64, (ImageSource, Image<'c>)>,
}

impl<'c> ImageCache<'c> {
    pub fn new(ctx: &'c Context) -> Self {
        Self { ctx, map: HashMap::new() }
    }

    /// Chamar antes de pintar e depois do envio do frame anterior:
    /// solta imagens órfãs e envia as novas da árvore.
    pub fn prepare<V>(&mut self, tree: &Tree<V>) {
        self.map.retain(|_, (src, _)| Rc::strong_count(src) > 1);
        for n in &tree.nodes {
            if let Some(g) = &n.style.gradient {
                self.upload(&super::gradient::ramp(g));
            }
            match &n.kind {
                NodeKind::Image(src) | NodeKind::Icon(_, Some(src)) | NodeKind::Polygon(_, _, Some(src)) => self.upload(src),
                _ => {}
            }
        }
    }

    /// Envia `src` se tem pixels pendentes (nova, ou quadro novo de imagem dinâmica).
    fn upload(&mut self, src: &ImageSource) {
        if let Some(ext) = src.external_source() {
            match ext.image(self.ctx) {
                Some(raw) => {
                    self.map.insert(src.id(), (src.clone(), Image::borrowed(raw)));
                }
                None => {
                    self.map.remove(&src.id());
                }
            }
            return;
        }
        let Some(pixels) = src.take_pixels() else { return };
        let (w, h) = src.size();
        if let Some((_, img)) = self.map.get_mut(&src.id()) {
            if img.size() == (w, h) {
                if let Err(e) = img.update_premultiplied(&pixels) {
                    eprintln!("higanbana-gpui: atualizar imagem falhou: {e}");
                }
                src.recycle(pixels);
                return;
            }
            self.map.remove(&src.id());
        }
        let made = if src.is_dynamic() {
            let made = Image::from_premultiplied(self.ctx, w, h, &pixels);
            src.recycle(pixels);
            made
        } else {
            Image::from_rgba8(self.ctx, w, h, pixels)
        };
        match made {
            Ok(img) => {
                self.map.insert(src.id(), (src.clone(), img));
            }
            Err(e) => eprintln!("higanbana-gpui: imagem {}x{} falhou: {e}", w, h),
        }
    }

    pub fn get(&self, src: &ImageSource) -> Option<&Image<'c>> {
        self.map.get(&src.id()).map(|(_, img)| img)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
