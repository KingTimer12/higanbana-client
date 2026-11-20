//! Árvore de elementos descrita por `View::render` a cada reconstrução.
mod id;
mod image_data;

pub use id::ElementId;
pub use image_data::{ImageData, ImageSource};

use crate::cx::EventCx;
use crate::gfx::{Modifiers, MouseButton};
use crate::geometry::Point;
use crate::style::{Overflow, Style, Styled};
use std::borrow::Cow;

/// Clique entregue aos handlers; posição em pixels lógicos da janela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseEvent {
    pub position: Point,
    pub button: MouseButton,
    pub mods: Modifiers,
}

/// Handler de clique tipado na view.
pub type ClickHandler<V> = Box<dyn Fn(&mut V, &MouseEvent, &mut EventCx)>;
/// Transforma o estilo base no estilo de hover (só propriedades visuais valem).
pub type HoverStyle = Box<dyn Fn(Style) -> Style>;

pub(crate) enum Kind {
    Div,
    Text(Cow<'static, str>),
    Image(ImageSource),
}

/// Elemento: div (com filhos), texto, imagem ou contêiner de rolagem.
pub struct Element<V> {
    pub(crate) kind: Kind,
    pub(crate) style: Style,
    pub(crate) id: Option<ElementId>,
    pub(crate) hover: Option<HoverStyle>,
    pub(crate) on_click: Option<ClickHandler<V>>,
    pub(crate) children: Vec<Element<V>>,
}

fn element<V>(kind: Kind, style: Style) -> Element<V> {
    Element { kind, style, id: None, hover: None, on_click: None, children: Vec::new() }
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

    /// Clique = botão desce e sobe sobre o mesmo elemento.
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
