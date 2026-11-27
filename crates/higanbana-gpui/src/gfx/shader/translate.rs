//! Módulo WGSL composto -> HLSL, GLSL 330 (um por estágio) e SPIR-V, via naga.
//!
//! Mapa de bindings para os backends (o WGSL tem `@group(0)` com 0 uniforms, 1 textura, 2 sampler):
//! - HLSL: `cbuffer hg_u : register(b0)`, `Texture2D<float4> hg_backdrop : register(t1)`,
//!   `SamplerState hg_backdrop_sampler : register(s2)`; entradas `vs_main`/`ps_main` num só fonte.
//! - GLSL 330 (sem `layout(binding)`): o backend liga pelos nomes abaixo — blocos de uniforms
//!   `GLSL_UNIFORM_BLOCK_VS`/`_FS` (ambos no ponto de ligação 0 via `glUniformBlockBinding`) e
//!   o `sampler2D` combinado `GLSL_BACKDROP_SAMPLER` (unidade de textura do backdrop). Sem
//!   ajuste de espaço de coordenadas: `gl_Position` sai igual ao WGSL, como os shaders GL atuais.
//! - SPIR-V: `vs_main` e `ps_main` no mesmo módulo, descriptor set 0 bindings 0/1/2; Y ajustado
//!   para o clip space do Vulkan (mesmas opções do build.rs do -sys).
//! - WGSL: o módulo composto inteiro (o backend WGPU compila direto).

/// Fontes prontos para `hg_shader_code`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Translations {
    pub hlsl: String,
    pub glsl_vs: String,
    pub glsl_fs: String,
    pub spirv: Vec<u32>,
    pub wgsl: String,
}

/// Nome do bloco de uniforms no GLSL do vértice.
pub const GLSL_UNIFORM_BLOCK_VS: &str = "HgShaderUniforms_block_0Vertex";
/// Nome do bloco de uniforms no GLSL do fragmento.
pub const GLSL_UNIFORM_BLOCK_FS: &str = "HgShaderUniforms_block_0Fragment";
/// `sampler2D` do backdrop no GLSL do fragmento (textura + sampler combinados).
pub const GLSL_BACKDROP_SAMPLER: &str = "_group_0_binding_1_fs";

#[cfg(feature = "shaders")]
pub(crate) use imp::translate;

#[cfg(feature = "shaders")]
mod imp {
    use super::super::error::ShaderError;
    use super::super::locate::UserRegion;
    use super::super::{contract, hlsl};
    use super::Translations;
    use naga::back::{glsl, spv};
    use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};
    use naga::{Module, ShaderStage};

    /// Traduz o módulo composto `wgsl`; erros apontam para `region` (código do usuário).
    pub(crate) fn translate(wgsl: String, region: UserRegion) -> Result<Translations, ShaderError> {
        let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| {
            let (line, column) = region.locate(e.location(&wgsl));
            ShaderError::Parse { line, column, message: e.message().to_owned() }
        })?;
        contract::check(&module, &wgsl, region)?;
        let info = Validator::new(ValidationFlags::all(), Capabilities::empty()).validate(&module).map_err(|e| {
            let (line, column) = region.locate(e.location(&wgsl));
            ShaderError::Validate { line, column, message: chain(e.as_inner()) }
        })?;
        Ok(Translations {
            hlsl: hlsl::write(&module, &info)?,
            glsl_vs: write_glsl(&module, &info, ShaderStage::Vertex, "vs_main")?,
            glsl_fs: write_glsl(&module, &info, ShaderStage::Fragment, "ps_main")?,
            spirv: spv::write_vec(&module, &info, &spv::Options::default(), None)
                .map_err(|e| ShaderError::Native(format!("naga (SPIR-V): {e}")))?,
            wgsl,
        })
    }

    /// Mensagem com a cadeia de causas (a validação do naga aninha o motivo real).
    fn chain(e: &dyn std::error::Error) -> String {
        let mut s = e.to_string();
        let mut cur = e.source();
        while let Some(c) = cur {
            s.push_str(": ");
            s.push_str(&c.to_string());
            cur = c.source();
        }
        s
    }

    fn write_glsl(module: &Module, info: &ModuleInfo, stage: ShaderStage, entry: &str) -> Result<String, ShaderError> {
        let options = glsl::Options {
            version: glsl::Version::Desktop(330),
            writer_flags: glsl::WriterFlags::INCLUDE_UNUSED_ITEMS,
            ..Default::default()
        };
        let pipeline = glsl::PipelineOptions { shader_stage: stage, entry_point: entry.into(), multiview: None };
        let mut out = String::new();
        glsl::Writer::new(&mut out, module, info, &options, &pipeline, Default::default())
            .and_then(|mut w| w.write())
            .map_err(|e| ShaderError::Native(format!("naga (GLSL {entry}): {e}")))?;
        Ok(out)
    }
}
