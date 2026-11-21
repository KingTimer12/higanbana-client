//! Árvore viva entre frames: nós achatados, layout, rolagem, foco, overlays e entrada.
pub(crate) mod node;
mod overlay;
mod relayout;
#[cfg(test)]
mod tests_transform;
#[cfg(test)]
mod tests_hit_regions;
mod hit_regions;
#[cfg(test)]
mod tests_scroll;
mod icons;
mod scroll;
mod transform;

use crate::element::{Element, ElementId};
use crate::geometry::{Point, Rect, Size};
use crate::layout::{LayoutEngine, ScrollOffsets, TextMeasurer};
use crate::style::TextStyle;
use node::{Node, OverlayRoot};
use std::collections::HashMap;

pub use overlay::place_overlay;

/// Identidade de um nó que sobrevive à reconstrução quando ele tem `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKey {
    Id(ElementId),
    Index(u32),
}

/// Estado de UI que a `App` mantém; usável sem GPU (testes).
pub struct Tree<V> {
    pub(crate) nodes: Vec<Node<V>>,
    pub(crate) overlays: Vec<OverlayRoot<V>>,
    pub(crate) engine: LayoutEngine,
    pub(crate) offsets: ScrollOffsets,
    /// Rolagem horizontal dos campos de texto, por id.
    pub(crate) field_scroll: HashMap<ElementId, f32>,
    pub(crate) viewport: Size,
    /// Última posição do mouse (lógica), se dentro da janela.
    pub(crate) mouse: Option<Point>,
    /// Nó mais profundo sob o mouse.
    pub(crate) hovered: Option<u32>,
    /// Nó clicável que recebeu o botão pressionado.
    pub(crate) pressed: Option<u32>,
    /// Nó que capturou o ponteiro (arraste) até o botão subir.
    pub(crate) captured: Option<NodeKey>,
    /// Botão esquerdo está segurado.
    pub(crate) button_down: bool,
    /// Elementos com `on_hover` atualmente sob o mouse.
    pub(crate) hover_owners: Vec<NodeKey>,
    pub(crate) focused: Option<ElementId>,
    pub(crate) focus_visible: bool,
    /// Primeiro nó que recebe entrada (raiz do overlay modal do topo, ou 0).
    pub(crate) barrier: usize,
}

impl<V> Default for Tree<V> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            overlays: Vec::new(),
            engine: LayoutEngine::default(),
            offsets: ScrollOffsets::default(),
            field_scroll: HashMap::new(),
            viewport: Size::default(),
            mouse: None,
            hovered: None,
            pressed: None,
            captured: None,
            button_down: false,
            hover_owners: Vec::new(),
            focused: None,
            focus_visible: false,
            barrier: 0,
        }
    }
}

impl<V> Tree<V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Troca a árvore de elementos e refaz o layout inteiro.
    pub fn rebuild(&mut self, root: Element<V>, base: &TextStyle, viewport: Size, measurer: &mut dyn TextMeasurer) {
        node::flatten(root, base, &mut self.nodes, &mut self.overlays);
        self.pressed = self.pressed.filter(|&p| (p as usize) < self.nodes.len());
        self.barrier = self.overlays.iter().rev().find(|o| o.spec.modal).map_or(0, |o| o.root as usize);
        self.relayout(viewport, measurer);
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

    /// Caixa absoluta do nó `i` (pré-ordem; overlays depois da árvore principal).
    pub fn rect(&self, i: usize) -> Rect {
        self.nodes[i].rect
    }

    /// Índice do primeiro nó com o `id` dado.
    /// Texto que será pintado no nó `i` (já cortado pelo `line_clamp`).
    #[cfg(test)]
    pub(crate) fn text_shown(&self, i: usize) -> &str {
        match &self.nodes[i].kind {
            node::NodeKind::Text(s) => self.nodes[i].shown.as_deref().unwrap_or(s),
            _ => "",
        }
    }

    pub fn find(&self, id: impl Into<ElementId>) -> Option<usize> {
        let id = Some(id.into());
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Deslocamento de rolagem atual do contêiner `id`.
    pub fn scroll_offset(&self, id: impl Into<ElementId>) -> f32 {
        self.offsets.get(&id.into()).copied().unwrap_or(0.0)
    }

    /// Quantos overlays a última reconstrução produziu.
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    /// Fim (exclusivo) da árvore principal; overlays começam aqui.
    pub(crate) fn main_end(&self) -> usize {
        self.overlays.first().map_or(self.nodes.len(), |o| o.root as usize)
    }

    pub(crate) fn key_of(&self, i: usize) -> NodeKey {
        self.nodes[i].id.map_or(NodeKey::Index(i as u32), NodeKey::Id)
    }

    pub(crate) fn resolve(&self, key: NodeKey) -> Option<usize> {
        match key {
            NodeKey::Id(id) => self.find(id),
            NodeKey::Index(i) => ((i as usize) < self.nodes.len()).then_some(i as usize),
        }
    }
}
