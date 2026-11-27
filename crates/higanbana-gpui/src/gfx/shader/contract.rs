//! Regras do contrato: o usuário só escreve funções; bindings e entradas são da lib.
use super::error::ShaderError;
use super::locate::UserRegion;
use naga::{AddressSpace, Module};

/// Globais da lib com binding (`@group(0) @binding(0..=2)`).
const LIB_GLOBALS: [&str; 3] = ["hg_u", "hg_backdrop", "hg_backdrop_sampler"];
/// Pontos de entrada da lib.
const LIB_ENTRIES: [&str; 2] = ["vs_main", "ps_main"];

pub(crate) fn check(module: &Module, wgsl: &str, region: UserRegion) -> Result<(), ShaderError> {
    for (handle, global) in module.global_variables.iter() {
        let is_lib = global.name.as_deref().is_some_and(|n| LIB_GLOBALS.contains(&n));
        let resource = global.binding.is_some() || matches!(global.space, AddressSpace::Immediate);
        if resource && !is_lib {
            let (line, column) = region.locate_span(module.global_variables.get_span(handle), wgsl);
            return Err(ShaderError::Validate {
                line,
                column,
                message: "shader do usuário não declara binding nem immediate; use param(i) e backdrop()".into(),
            });
        }
    }
    if let Some(ep) = module.entry_points.iter().find(|ep| !LIB_ENTRIES.contains(&ep.name.as_str())) {
        return Err(ShaderError::Validate {
            line: 0,
            column: 0,
            message: format!("ponto de entrada `{}` não permitido: escreva só `fn shade(p: ShadeInput) -> vec4<f32>`", ep.name),
        });
    }
    Ok(())
}
