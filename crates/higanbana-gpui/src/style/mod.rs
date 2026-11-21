//! Estilo de elementos: layout (flexbox), visual e texto herdável.
mod gradient;
mod styled;
mod text;

pub use gradient::{GradientDir, LinearGradient};
pub use styled::Styled;
pub use text::{TextStyle, TextStyleRefinement};

use crate::color::Color;
use crate::geometry::{Corners, Edges, Point};
use crate::gfx::CursorStyle;

/// Como uma imagem ocupa a caixa: esticada, inteira (barras) ou preenchendo (corta).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectFit {
    #[default]
    Fill,
    Contain,
    Cover,
}

/// Comprimento em pixels lógicos, porcentagem do pai ou automático.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    #[default]
    Auto,
    Px(f32),
    /// 0..100.
    Percent(f32),
}

pub const fn px(v: f32) -> Length {
    Length::Px(v)
}

/// Porcentagem do pai (0..100).
pub const fn pct(v: f32) -> Length {
    Length::Percent(v)
}

pub const fn auto() -> Length {
    Length::Auto
}

impl From<f32> for Length {
    fn from(v: f32) -> Self {
        Length::Px(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Flex,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Alinhamento no eixo cruzado (`align-items` / `align-self`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

/// Distribuição no eixo principal (`justify-content`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    /// Recorta os filhos na caixa do elemento.
    Hidden,
    /// Recorta e rola verticalmente (precisa de `id`).
    Scroll,
    /// Recorta e rola horizontalmente (precisa de `id`).
    ScrollX,
}

impl Overflow {
    /// Contêiner de rolagem (qualquer eixo)?
    pub fn is_scroll(self) -> bool {
        matches!(self, Overflow::Scroll | Overflow::ScrollX)
    }
}

/// Sombra difusa ao redor da caixa (sem deslocamento, como o core).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Shadow {
    pub color: Color,
    pub blur: f32,
}

/// Estilo completo de um elemento. Construído pelos métodos de [`Styled`].
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub display: Display,
    pub direction: Direction,
    pub wrap: bool,
    pub gap_x: f32,
    pub gap_y: f32,
    pub padding: Edges<f32>,
    pub margin: Edges<Length>,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    pub justify: Option<Justify>,
    pub position: Position,
    pub inset: Edges<Length>,
    pub overflow: Overflow,
    pub background: Option<Color>,
    /// Gradiente pintado por cima do `background`.
    pub gradient: Option<LinearGradient>,
    pub border_color: Color,
    pub border_width: f32,
    pub radius: Corners<f32>,
    pub shadow: Option<Shadow>,
    /// 0..1, multiplica a opacidade do elemento e dos filhos.
    pub opacity: f32,
    /// Deslocamento visual da subárvore (não afeta o layout dos irmãos).
    pub translate: Point,
    /// Escala visual da subárvore em torno do centro (1 = normal).
    pub scale: f32,
    /// Papel do elemento numa janela com barra customizada (arrastar, maximizar...).
    pub window_hit: Option<crate::gfx::HitKind>,
    /// Encaixe de imagens na caixa de conteúdo.
    pub object_fit: ObjectFit,
    /// Cursor do mouse sobre o elemento (o mais profundo com cursor vence).
    pub cursor: Option<CursorStyle>,
    /// Propriedades de texto herdadas pelos descendentes.
    pub text: TextStyleRefinement,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            direction: Direction::Row,
            wrap: false,
            gap_x: 0.0,
            gap_y: 0.0,
            padding: Edges::all(0.0),
            margin: Edges::all(Length::Px(0.0)),
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            align_items: None,
            align_self: None,
            justify: None,
            position: Position::Relative,
            inset: Edges::all(Length::Auto),
            overflow: Overflow::Visible,
            background: None,
            gradient: None,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: Corners::all(0.0),
            shadow: None,
            opacity: 1.0,
            translate: Point { x: 0.0, y: 0.0 },
            scale: 1.0,
            window_hit: None,
            object_fit: ObjectFit::Fill,
            cursor: None,
            text: TextStyleRefinement::default(),
        }
    }
}

impl Style {
    /// Tem translate ou scale?
    pub fn has_transform(&self) -> bool {
        self.scale != 1.0 || self.translate.x != 0.0 || self.translate.y != 0.0
    }

    /// Tem algo a pintar como quad (fundo, borda ou sombra)?
    pub fn has_box(&self) -> bool {
        self.background.is_some_and(|c| !c.is_transparent())
            || (self.border_width > 0.0 && !self.border_color.is_transparent())
            || self.shadow.is_some_and(|s| !s.color.is_transparent())
    }
}
