//! `ShaderFill`: shader + parâmetros usados como preenchimento de um elemento.
use super::handle::Shader;
use super::ShaderParams;

/// Preenchimento por shader (`div().shader(&s, params![...])`); substitui cor/gradiente.
#[derive(Debug, Clone)]
pub struct ShaderFill {
    pub shader: Shader,
    pub params: ShaderParams,
}

impl PartialEq for ShaderFill {
    /// Mesmo shader (mesma tradução e flags) e mesmos parâmetros.
    fn eq(&self, other: &Self) -> bool {
        self.shader.same(&other.shader)
            && self.shader.is_backdrop() == other.shader.is_backdrop()
            && self.shader.is_animated() == other.shader.is_animated()
            && self.params == other.params
    }
}
