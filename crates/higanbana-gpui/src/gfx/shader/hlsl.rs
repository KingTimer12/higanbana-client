//! WGSL -> HLSL (SM 5.0) com registradores fixos: b0 uniforms, t1 backdrop, s2 sampler.
use super::error::ShaderError;
use naga::back::hlsl;
use naga::valid::ModuleInfo;
use naga::{Module, ResourceBinding};

/// Registrador `s` do sampler do backdrop.
pub const HLSL_SAMPLER_REGISTER: u32 = 2;

fn target(register: u32) -> hlsl::BindTarget {
    hlsl::BindTarget { space: 0, register, binding_array_size: None, dynamic_storage_buffer_offsets_index: None, restrict_indexing: false }
}

pub(crate) fn write(module: &Module, info: &ModuleInfo) -> Result<String, ShaderError> {
    let mut options = hlsl::Options { shader_model: hlsl::ShaderModel::V5_0, fake_missing_bindings: false, ..Default::default() };
    for binding in 0..3 {
        options.binding_map.insert(ResourceBinding { group: 0, binding }, target(binding));
    }
    // Exigido pelo naga; a declaração resultante é removida em `plain_sampler`.
    options.sampler_buffer_binding_map.insert(hlsl::SamplerIndexBufferKey { group: 0 }, target(0));
    let pipeline = hlsl::PipelineOptions { entry_point: None };
    let mut out = String::new();
    hlsl::Writer::new(&mut out, &options, &pipeline)
        .write(module, info, None)
        .map_err(|e| ShaderError::Native(format!("naga (HLSL): {e}")))?;
    plain_sampler(&out)
}

/// O naga emite samplers via "sampler heap" (`nagaSamplerHeap[2048]`, índice num
/// `StructuredBuffer` em `space255`), coisa de D3D12/SM 5.1. Para `vs_5_0`/`ps_5_0`
/// no D3D11 e no D3D12 o sampler vira um `SamplerState` comum em `s2`.
fn plain_sampler(src: &str) -> Result<String, ShaderError> {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("SamplerState nagaSamplerHeap")
            || t.starts_with("SamplerComparisonState nagaComparisonSamplerHeap")
            || t.starts_with("StructuredBuffer<uint> nagaGroup")
        {
            continue;
        }
        let sampler = t.strip_prefix("static const SamplerState ").and_then(|r| r.split_once(" = nagaSamplerHeap["));
        if let Some((name, _)) = sampler {
            out.push_str(&format!("SamplerState {name} : register(s{HLSL_SAMPLER_REGISTER});\n"));
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if out.contains("nagaSamplerHeap") || out.contains("nagaComparisonSamplerHeap") {
        return Err(ShaderError::Native("naga (HLSL): sampler heap inesperado".into()));
    }
    Ok(out)
}
