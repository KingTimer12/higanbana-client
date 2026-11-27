//! Converte posições do módulo composto para o código do usuário.
use naga::{SourceLocation, Span};

/// Onde o código do usuário está dentro do módulo composto.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UserRegion {
    /// Linhas do prelúdio antes do código do usuário.
    pub prelude_lines: u32,
    /// Linhas do código do usuário.
    pub user_lines: u32,
}

impl UserRegion {
    pub fn new(prelude_lines: u32, user: &str) -> Self {
        Self { prelude_lines, user_lines: user.lines().count() as u32 }
    }

    /// (linha, coluna) no código do usuário, 1-based; (0, 0) = fora dele (código da lib).
    pub fn locate(&self, loc: Option<SourceLocation>) -> (u32, u32) {
        match loc {
            Some(l) if l.line_number > self.prelude_lines && l.line_number - self.prelude_lines <= self.user_lines => {
                (l.line_number - self.prelude_lines, l.line_position)
            }
            _ => (0, 0),
        }
    }

    pub fn locate_span(&self, span: Span, source: &str) -> (u32, u32) {
        self.locate(span.is_defined().then(|| span.location(source)))
    }
}
