//! Shaders WGSL do usuário: `fn shade(p: ShadeInput) -> vec4<f32>` montado com o prelúdio
//! da lib e traduzido pelo naga (feature `shaders`) para HLSL, GLSL e SPIR-V.
#[cfg(feature = "shaders")]
mod compose;
#[cfg(feature = "shaders")]
mod contract;
mod error;
mod fill;
mod gpu;
mod handle;
#[cfg(feature = "shaders")]
mod hlsl;
#[cfg(feature = "shaders")]
mod locate;
mod translate;

pub use error::ShaderError;
pub use fill::ShaderFill;
pub use gpu::{GpuShader, ShaderDraw};
pub use handle::Shader;
pub use translate::{Translations, GLSL_BACKDROP_SAMPLER, GLSL_UNIFORM_BLOCK_FS, GLSL_UNIFORM_BLOCK_VS};

/// Até 16 `vec4` por desenho (`param(i)` no WGSL).
pub type ShaderParams = [[f32; 4]; 16];

/// `params![a, b, ...]`: cada valor vira `[f32; 4]` (`Into`), o resto fica zero.
/// Mais de 16 valores entra em pânico.
#[macro_export]
macro_rules! params {
    ($($v:expr),* $(,)?) => {{
        let mut p: $crate::gfx::shader::ShaderParams = [[0.0f32; 4]; 16];
        let v: &[[f32; 4]] = &[$(::core::convert::Into::<[f32; 4]>::into($v)),*];
        p[..v.len()].copy_from_slice(v);
        p
    }};
}

#[cfg(test)]
mod tests;
