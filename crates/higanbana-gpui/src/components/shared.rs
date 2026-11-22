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
    /// `xsmall` do kit: 20 px, texto xs (botões de barra e listas).
    Xs,
    /// `medium` do kit: 32 px (botões do player).
    Regular,
    /// `small` do kit: 24 px, texto sm (title bar, sidebar, menus).
    Compact,
}

impl ComponentSize {
    /// Altura em pixels lógicos (h-8 / h-9 / h-10; kit: h-5 / h-6).
    pub fn height(self) -> f32 {
        match self {
            Self::Sm | Self::Regular => 32.0,
            Self::Md | Self::Icon => 36.0,
            Self::Lg => 40.0,
            Self::Xs => 20.0,
            Self::Compact => 24.0,
        }
    }
}
