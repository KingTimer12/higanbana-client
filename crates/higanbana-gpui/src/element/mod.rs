//! Árvore de elementos descrita por `View::render` a cada reconstrução.
mod handlers;
mod id;
mod image_data;
mod interactive;
mod overlay;
mod text_field;

pub use handlers::{
    CharHandler, ClickHandler, DismissHandler, HoverHandler, HoverStyle, KeyHandler, MouseEvent, PointerEvent,
    PointerHandler, StyleFn,
};
pub use id::ElementId;
pub use image_data::{ImageData, ImageSource};
pub use overlay::{overlay, Anchor, AnchorAlign, Overlay, Side};
pub use text_field::{text_field, CARET_BLINK_MS};

pub(crate) use handlers::Interactivity;
pub(crate) use overlay::OverlaySpec;
pub(crate) use text_field::FieldData;

use crate::cx::EventCx;
use crate::style::{Overflow, Style, Styled};
use std::borrow::Cow;

pub(crate) enum Kind<V> {
    Div,
    Text(Cow<'static, str>),
    Image(ImageSource),
    /// Ícone SVG registrado (ver `crate::icon`).
    Icon(&'static str),
    TextField(Box<FieldData>),
    /// Conteúdo (único filho) vai para a camada de overlays.
    Overlay(Box<OverlaySpec<V>>),
}

/// Elemento: div (com filhos), texto, imagem ou contêiner de rolagem.
pub struct Element<V> {
    pub(crate) kind: Kind<V>,
    pub(crate) style: Style,
    pub(crate) id: Option<ElementId>,
    pub(crate) hover: Option<HoverStyle>,
    pub(crate) on_click: Option<ClickHandler<V>>,
    pub(crate) interact: Option<Box<Interactivity<V>>>,
    pub(crate) children: Vec<Element<V>>,
}

pub(crate) fn element<V>(kind: Kind<V>, style: Style) -> Element<V> {
    Element { kind, style, id: None, hover: None, on_click: None, interact: None, children: Vec::new() }
}

/// Caixa flex vazia.
pub fn div<V>() -> Element<V> {
    element(Kind::Div, Style::default())
}

/// Texto; herda fonte/cor/tamanho dos ancestrais.
pub fn text<V>(content: impl Into<Cow<'static, str>>) -> Element<V> {
    element(Kind::Text(content.into()), Style::default())
}

/// Imagem; sem tamanho explícito usa o tamanho natural (proporção mantida).
pub fn img<V>(source: ImageSource) -> Element<V> {
    element(Kind::Image(source), Style::default())
}

/// Coluna com rolagem vertical; o deslocamento persiste pelo `id` entre reconstruções.
pub fn scroll<V>(id: impl Into<ElementId>) -> Element<V> {
    let mut e = div().flex_col().id(id);
    e.style.overflow = Overflow::Scroll;
    e
}

impl<V> Element<V> {
    /// Identidade estável (estado de rolagem, hover) entre reconstruções.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn child(mut self, child: impl Into<Element<V>>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<E: Into<Element<V>>>(mut self, iter: impl IntoIterator<Item = E>) -> Self {
        self.children.extend(iter.into_iter().map(Into::into));
        self
    }

    /// Estilo aplicado enquanto o mouse está sobre o elemento (sem relayout).
    pub fn hover(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.hover = Some(Box::new(f));
        self
    }

    /// Clique = botão desce e sobe sobre o mesmo elemento. Com foco, Enter/Espaço
    /// não tratados também disparam o clique.
    pub fn on_click(mut self, f: impl Fn(&mut V, &MouseEvent, &mut EventCx) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Aplica `f` só quando `cond` é verdadeiro (encadeamento condicional).
    pub fn when(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond { f(self) } else { self }
    }

    pub fn style(&self) -> &Style {
        &self.style
    }
}

impl<V> Styled for Element<V> {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<V> From<&'static str> for Element<V> {
    fn from(s: &'static str) -> Self {
        text(s)
    }
}

impl<V> From<String> for Element<V> {
    fn from(s: String) -> Self {
        text(s)
    }
}
