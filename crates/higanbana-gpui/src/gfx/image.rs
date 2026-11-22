//! Imagens RGBA8 na GPU, presas ao tempo de vida do `Context`.
use super::context::Context;
use crate::color::premultiply_rgba8;
use crate::error::{check, Error, Result};
use higanbana_gpui_sys as sys;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Imagem do core. O core guarda a própria cópia dos pixels (reenvio na troca de backend),
/// então quem chama pode soltar o buffer logo depois de criar.
pub struct Image<'c> {
    raw: NonNull<sys::hg_image>,
    width: u32,
    height: u32,
    /// `false` = imagem de outro dono (alvo de vídeo): não é destruída aqui.
    owned: bool,
    _ctx: PhantomData<&'c Context>,
}

impl<'c> Image<'c> {
    /// Pixels RGBA8 com alfa reto; pré-multiplica no próprio buffer (sem cópia extra).
    pub fn from_rgba8(ctx: &'c Context, width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Self> {
        premultiply_rgba8(&mut pixels);
        Self::from_premultiplied(ctx, width, height, &pixels)
    }

    /// Pixels RGBA8 já pré-multiplicados, linhas contíguas.
    pub fn from_premultiplied(ctx: &'c Context, width: u32, height: u32, pixels: &[u8]) -> Result<Self> {
        let needed = width as usize * height as usize * 4;
        if pixels.len() < needed {
            return Err(Error::invalid("buffer de imagem menor que width*height*4"));
        }
        let mut raw = std::ptr::null_mut();
        check(unsafe { sys::hg_image_create(ctx.raw(), width, height, pixels.as_ptr(), width * 4, &mut raw) })?;
        let raw = NonNull::new(raw).ok_or_else(|| Error::invalid("hg_image_create devolveu nulo"))?;
        Ok(Self { raw, width, height, owned: true, _ctx: PhantomData })
    }

    /// Imagem do core que pertence a outro (alvo de vídeo); só desenhada daqui.
    pub(crate) fn borrowed(raw: NonNull<sys::hg_image>) -> Self {
        let (mut width, mut height) = (0, 0);
        unsafe { sys::hg_image_size(raw.as_ptr(), &mut width, &mut height) };
        Self { raw, width, height, owned: false, _ctx: PhantomData }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Troca os pixels (pré-multiplicados, mesmo tamanho) sem recriar a textura.
    pub fn update_premultiplied(&mut self, pixels: &[u8]) -> Result<()> {
        if pixels.len() < self.width as usize * self.height as usize * 4 {
            return Err(Error::invalid("buffer de imagem menor que width*height*4"));
        }
        check(unsafe { sys::hg_image_update(self.raw.as_ptr(), pixels.as_ptr(), self.width * 4) })
    }

    /// Identidade da imagem no core (testes: atualizada no lugar vs recriada).
    pub fn raw_ptr(&self) -> usize {
        self.raw.as_ptr() as usize
    }

    pub(crate) fn raw(&self) -> *mut sys::hg_image {
        self.raw.as_ptr()
    }
}

impl Drop for Image<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { sys::hg_image_destroy(self.raw.as_ptr()) };
        }
    }
}
