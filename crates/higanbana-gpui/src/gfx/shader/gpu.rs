//! `GpuShader`: o `hg_shader` do core, criado pelo `Context` a partir de um `Shader`.
use super::handle::Shader;
use super::ShaderParams;
use crate::error::{check, Error, Result};
use crate::geometry::{Corners, Rect};
use crate::gfx::context::Context;
use higanbana_gpui_sys as sys;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Um desenho de shader: retângulo final em pixels físicos e os valores do quadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShaderDraw {
    pub rect: Rect,
    pub radius: Corners<f32>,
    /// Segundos (normalmente `Shader::time()`).
    pub time: f32,
    pub scale: f32,
    pub opacity: f32,
    pub params: ShaderParams,
}

impl Default for ShaderDraw {
    fn default() -> Self {
        Self { rect: Rect::default(), radius: Corners::default(), time: 0.0, scale: 1.0, opacity: 1.0, params: [[0.0; 4]; 16] }
    }
}

/// Shader no core, preso ao tempo de vida do `Context`. O core guarda a própria cópia
/// dos fontes e compila sob demanda no backend ativo (de novo após troca de backend).
pub struct GpuShader<'c> {
    raw: NonNull<sys::hg_shader>,
    backdrop: bool,
    _ctx: PhantomData<&'c Context>,
}

fn cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| Error::invalid("fonte de shader com byte nulo"))
}

impl Context {
    /// Envia as traduções de `s` ao core.
    pub fn shader(&self, s: &Shader) -> Result<GpuShader<'_>> {
        let t = s.translations();
        let (hlsl, glsl_vs, glsl_fs, wgsl) = (cstring(&t.hlsl)?, cstring(&t.glsl_vs)?, cstring(&t.glsl_fs)?, cstring(&t.wgsl)?);
        let code = sys::hg_shader_code {
            hlsl: hlsl.as_ptr(),
            glsl_vs: glsl_vs.as_ptr(),
            glsl_fs: glsl_fs.as_ptr(),
            wgsl: wgsl.as_ptr(),
            spirv: t.spirv.as_ptr(),
            spirv_words: t.spirv.len(),
        };
        let mut raw = std::ptr::null_mut();
        check(unsafe { sys::hg_shader_create(self.raw(), &code, &mut raw) })?;
        let raw = NonNull::new(raw).ok_or_else(|| Error::invalid("hg_shader_create devolveu nulo"))?;
        Ok(GpuShader { raw, backdrop: s.is_backdrop(), _ctx: PhantomData })
    }
}

impl GpuShader<'_> {
    /// Erro do compilador nativo no backend atual, se houve.
    pub fn native_error(&self) -> Option<String> {
        let p = unsafe { sys::hg_shader_error(self.raw.as_ptr()) };
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }

    pub(crate) fn raw(&self) -> *mut sys::hg_shader {
        self.raw.as_ptr()
    }

    pub(crate) fn flags(&self) -> u32 {
        if self.backdrop { sys::HG_SHADER_BACKDROP } else { 0 }
    }
}

impl Drop for GpuShader<'_> {
    fn drop(&mut self) {
        unsafe { sys::hg_shader_destroy(self.raw.as_ptr()) };
    }
}
