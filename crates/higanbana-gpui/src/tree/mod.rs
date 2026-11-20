//! Árvore viva entre frames: nós achatados, layout, rolagem, hover e clique.
pub(crate) mod node;

use crate::element::Element;
use crate::geometry::{Point, Rect, Size};
use crate::layout::{position, LayoutEngine, ScrollOffsets, TextMeasurer};
use crate::style::TextStyle;
use node::Node;

/// Estado de UI que a `App` mantém; usável sem GPU (testes).
pub struct Tree<V> {
    pub(crate) nodes: Vec<Node<V>>,
    pub(crate) engine: LayoutEngine,
    pub(crate) offsets: ScrollOffsets,
    pub(crate) viewport: Size,
    /// Última posição do mouse (lógica), se dentro da janela.
    pub(crate) mouse: Option<Point>,
    /// Nó mais profundo sob o mouse.
    pub(crate) hovered: Option<u32>,
    /// Nó clicável que recebeu o botão pressionado.
    pub(crate) pressed: Option<u32>,
}

impl<V> Default for Tree<V> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            engine: LayoutEngine::default(),
            offsets: ScrollOffsets::default(),
            viewport: Size::default(),
            mouse: None,
            hovered: None,
            pressed: None,
        }
    }
}

impl<V> Tree<V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Troca a árvore de elementos e refaz o layout inteiro.
    pub fn rebuild(&mut self, root: Element<V>, base: &TextStyle, viewport: Size, measurer: &mut dyn TextMeasurer) {
        node::flatten(root, base, &mut self.nodes);
        self.pressed = self.pressed.filter(|&p| (p as usize) < self.nodes.len());
        self.relayout(viewport, measurer);
    }

    /// Refaz o layout dos mesmos nós (ex.: redimensionamento).
    pub fn relayout(&mut self, viewport: Size, measurer: &mut dyn TextMeasurer) {
        self.viewport = viewport;
        self.engine.compute(&self.nodes, viewport, measurer);
        self.reposition();
    }

    /// Só reposiciona (mudou a rolagem): não chama o taffy.
    pub(crate) fn reposition(&mut self) {
        position(&self.engine, &mut self.nodes, self.viewport, &mut self.offsets);
        self.hovered = self.mouse.and_then(|p| self.hit_test(p)).map(|i| i as u32);
    }

    /// Esquece medidas de texto (mudou DPI ou fonte).
    pub fn clear_measure_cache(&mut self) {
        self.engine.cache.clear();
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Caixa absoluta do nó `i` (pré-ordem).
    pub fn rect(&self, i: usize) -> Rect {
        self.nodes[i].rect
    }

    /// Índice do primeiro nó com o `id` dado.
    pub fn find(&self, id: impl Into<crate::element::ElementId>) -> Option<usize> {
        let id = Some(id.into());
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Deslocamento de rolagem atual do contêiner `id`.
    pub fn scroll_offset(&self, id: impl Into<crate::element::ElementId>) -> f32 {
        self.offsets.get(&id.into()).copied().unwrap_or(0.0)
    }
}
