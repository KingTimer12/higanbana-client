//! Tipos comuns entre componentes (variantes e tamanhos do shadcn).

/// Variante visual (`variant` do shadcn).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    #[default]
    Default,
    Secondary,
    Outline,
    Ghost,
    Destructive,
    Link,
}

/// Tamanho (`size` do shadcn). `Icon` = quadrado só com ícone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComponentSize {
    Sm,
    #[default]
    Md,
    Lg,
    Icon,
}

impl ComponentSize {
    /// Altura em pixels lógicos (h-8 / h-9 / h-10).
    pub fn height(self) -> f32 {
        match self {
            Self::Sm => 32.0,
            Self::Md | Self::Icon => 36.0,
            Self::Lg => 40.0,
        }
    }
}
