//! Erros de shader do usuário (linha/coluna sempre do código do usuário).
use std::fmt;

/// Falha ao criar ou compilar um `Shader`.
///
/// `line`/`column` começam em 1 e contam só o código do usuário; `line == 0` indica
/// que o erro caiu no código da lib (prelúdio ou pontos de entrada), por exemplo
/// quando falta `fn shade` ou a assinatura dela não bate.
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderError {
    /// Erro de sintaxe ou de tipo detectado ao ler o WGSL.
    Parse { line: u32, column: u32, message: String },
    /// Módulo lido, mas inválido (validação do naga ou regra do contrato).
    Validate { line: u32, column: u32, message: String },
    /// Tradução ou compilador nativo do backend (D3DCompile, driver GL, Vulkan, WGPU).
    Native(String),
    /// Feature `shaders` desligada.
    Unsupported,
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { line, column, message } => write!(f, "erro de sintaxe em {line}:{column}: {message}"),
            Self::Validate { line, column, message } => write!(f, "shader inválido em {line}:{column}: {message}"),
            Self::Native(message) => write!(f, "compilação nativa falhou: {message}"),
            Self::Unsupported => f.write_str("shaders desligados (feature `shaders`)"),
        }
    }
}

impl std::error::Error for ShaderError {}
