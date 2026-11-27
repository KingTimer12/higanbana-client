//! Draw list segura. Coordenadas aqui são físicas (o chamador já multiplicou pela escala).
use super::font::Font;
use super::image::Image;
use super::shader::{GpuShader, ShaderDraw};
use crate::color::Color;
use crate::error::{check, Result};
use crate::geometry::{Corners, Rect};
use higanbana_gpui_sys as sys;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// Retângulo arredondado com borda e sombra (SDF no core).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub fill: Color,
    pub border: Color,
    pub border_width: f32,
    pub radius: Corners<f32>,
    pub shadow: Color,
    pub shadow_blur: f32,
}

/// Lista de primitivas de um frame. `'a` amarra as imagens referenciadas:
/// elas vivem pelo menos até a lista ser enviada ou reciclada.
pub struct DrawList<'a> {
    raw: NonNull<sys::hg_draw_list>,
    _images: PhantomData<&'a ()>,
}

/// Clip ativo; desempilha ao sair de escopo.
pub struct ClipGuard<'l, 'a> {
    list: &'l mut DrawList<'a>,
}

fn rect(r: Rect) -> sys::hg_rect {
    sys::hg_rect { x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height }
}

impl<'a> DrawList<'a> {
    pub fn new() -> Self {
        let raw = NonNull::new(unsafe { sys::hg_draw_list_create() }).expect("hg_draw_list_create: sem memória");
        Self { raw, _images: PhantomData }
    }

    pub(crate) fn raw(&self) -> *const sys::hg_draw_list {
        self.raw.as_ptr()
    }

    /// Esvazia mantendo a memória da arena; solta o empréstimo das imagens antigas.
    pub fn recycle<'b>(self) -> DrawList<'b> {
        unsafe { sys::hg_draw_list_reset(self.raw.as_ptr()) };
        let raw = self.raw;
        std::mem::forget(self);
        DrawList { raw, _images: PhantomData }
    }

    pub fn quad(&mut self, q: &Quad) -> Result<()> {
        let raw = sys::hg_quad {
            rect: rect(q.rect),
            fill: q.fill.to_hg(),
            border: q.border.to_hg(),
            radius: q.radius.to_array(),
            border_width: q.border_width,
            shadow_blur: q.shadow_blur,
            shadow: q.shadow.to_hg(),
        };
        check(unsafe { sys::hg_draw_quad(self.raw.as_ptr(), &raw) })
    }

    /// Desenha `uv` (0..1) de `image` em `dest`, multiplicada por `tint`.
    pub fn image(&mut self, image: &'a Image<'_>, dest: Rect, uv: Rect, tint: Color, radius: Corners<f32>) -> Result<()> {
        let d = sys::hg_image_draw { rect: rect(dest), uv: rect(uv), tint: tint.to_hg(), radius: radius.to_array() };
        check(unsafe { sys::hg_draw_image(self.raw.as_ptr(), image.raw(), &d) })
    }

    /// Shader do usuário numa camada própria (o que veio antes fica atrás), herdando o clip.
    pub fn shader(&mut self, gpu: &'a GpuShader<'_>, d: &ShaderDraw) -> Result<()> {
        let raw = sys::hg_shader_draw {
            rect: rect(d.rect),
            radius: d.radius.to_array(),
            time: d.time,
            scale: d.scale,
            opacity: d.opacity,
            params: d.params,
            flags: gpu.flags(),
        };
        check(unsafe { sys::hg_draw_shader(self.raw.as_ptr(), gpu.raw(), &raw) })
    }

    /// Texto com canto superior esquerdo em (x, y); `max_width <= 0` não quebra linha.
    pub fn text(&mut self, font: Font, size_px: f32, x: f32, y: f32, max_width: f32, color: Color, text: &str) -> Result<()> {
        check(unsafe {
            sys::hg_draw_text(
                self.raw.as_ptr(),
                font.id,
                size_px,
                x,
                y,
                max_width,
                color.to_hg(),
                text.as_ptr().cast(),
                text.len(),
            )
        })
    }

    /// Tudo desenhado depois fica por cima de tudo antes.
    pub fn layer(&mut self) -> Result<()> {
        check(unsafe { sys::hg_draw_list_layer(self.raw.as_ptr()) })
    }

    /// Empilha um clip (intersectado com o atual pelo core).
    pub fn clip(&mut self, r: Rect) -> Result<ClipGuard<'_, 'a>> {
        check(unsafe { sys::hg_push_clip(self.raw.as_ptr(), rect(r)) })?;
        Ok(ClipGuard { list: self })
    }

    /// Versão manual de `clip` para percursos recursivos; pareie com `pop_clip`.
    pub fn push_clip(&mut self, r: Rect) -> Result<()> {
        check(unsafe { sys::hg_push_clip(self.raw.as_ptr(), rect(r)) })
    }

    pub fn pop_clip(&mut self) {
        unsafe { sys::hg_pop_clip(self.raw.as_ptr()) }
    }

    pub fn quad_count(&self) -> u32 {
        unsafe { sys::hg_draw_list_quad_count(self.raw.as_ptr()) }
    }
}

impl Default for DrawList<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DrawList<'_> {
    fn drop(&mut self) {
        unsafe { sys::hg_draw_list_destroy(self.raw.as_ptr()) };
    }
}

impl<'a> Deref for ClipGuard<'_, 'a> {
    type Target = DrawList<'a>;
    fn deref(&self) -> &DrawList<'a> {
        self.list
    }
}

impl<'a> DerefMut for ClipGuard<'_, 'a> {
    fn deref_mut(&mut self) -> &mut DrawList<'a> {
        self.list
    }
}

impl Drop for ClipGuard<'_, '_> {
    fn drop(&mut self) {
        self.list.pop_clip();
    }
}
