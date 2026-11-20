//! Monta a árvore taffy a partir dos nós achatados e calcula o layout.
use super::measure::{MeasureCache, TextMeasurer};
use super::style_map::to_taffy;
use crate::geometry::Size;
use crate::tree::node::{children, Node, NodeKind};
use taffy::{AvailableSpace, NodeId, TaffyTree};

/// Dono do `TaffyTree`: a alocação é reaproveitada entre frames (`clear`, não recria).
pub struct LayoutEngine {
    pub(crate) taffy: TaffyTree<u32>,
    pub(crate) ids: Vec<NodeId>,
    buf: Vec<NodeId>,
    pub(crate) cache: MeasureCache,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        let mut taffy = TaffyTree::new();
        // Sem arredondamento: o texto é desenhado na mesma largura em que foi medido.
        taffy.disable_rounding();
        Self { taffy, ids: Vec::new(), buf: Vec::new(), cache: MeasureCache::default() }
    }
}

impl LayoutEngine {
    /// Recria os nós taffy (filhos antes dos pais) e calcula para a viewport.
    pub(crate) fn compute<V>(&mut self, nodes: &[Node<V>], viewport: Size, measurer: &mut dyn TextMeasurer) {
        self.taffy.clear();
        self.ids.clear();
        if nodes.is_empty() {
            return;
        }
        self.ids.resize(nodes.len(), NodeId::from(0u64));
        for i in (0..nodes.len()).rev() {
            let mut style = to_taffy(&nodes[i].style);
            if i == 0 {
                fill_viewport(&mut style, viewport);
            }
            let id = match nodes[i].kind {
                NodeKind::Div => {
                    self.buf.clear();
                    self.buf.extend(children(nodes, i).map(|c| self.ids[c]));
                    self.taffy.new_with_children(style, &self.buf)
                }
                _ => self.taffy.new_leaf_with_context(style, i as u32),
            };
            self.ids[i] = id.expect("taffy: falha ao criar nó");
        }
        let space = taffy::Size {
            width: AvailableSpace::Definite(viewport.width),
            height: AvailableSpace::Definite(viewport.height),
        };
        let cache = &mut self.cache;
        let result = self.taffy.compute_layout_with_measure(self.ids[0], space, |input, _, ctx, style| {
            taffy::compute_leaf_layout(input, style, |_, _| 0.0, |known, avail| match ctx {
                Some(i) => measure_leaf(&nodes[*i as usize], known, avail, cache, measurer),
                None => taffy::Size::ZERO,
            })
        });
        result.expect("taffy: falha no layout");
    }

    pub(crate) fn layout(&self, i: usize) -> &taffy::Layout {
        self.taffy.layout(self.ids[i]).expect("taffy: nó sem layout")
    }
}

/// A raiz sem tamanho explícito ocupa a viewport inteira.
fn fill_viewport(style: &mut taffy::Style, viewport: Size) {
    if style.size.width == taffy::Dimension::auto() {
        style.size.width = taffy::Dimension::length(viewport.width);
    }
    if style.size.height == taffy::Dimension::auto() {
        style.size.height = taffy::Dimension::length(viewport.height);
    }
}

fn measure_leaf<V>(
    node: &Node<V>,
    known: taffy::Size<Option<f32>>,
    avail: taffy::Size<AvailableSpace>,
    cache: &mut MeasureCache,
    measurer: &mut dyn TextMeasurer,
) -> taffy::Size<f32> {
    if let taffy::Size { width: Some(width), height: Some(height) } = known {
        return taffy::Size { width, height };
    }
    match &node.kind {
        NodeKind::Text(s) => {
            let max_width = if !node.text.wrap {
                None
            } else {
                known.width.or(match avail.width {
                    AvailableSpace::Definite(w) => Some(w),
                    // Mínimo de conteúdo: quebra em toda palavra.
                    AvailableSpace::MinContent => Some(1.0),
                    AvailableSpace::MaxContent => None,
                })
            };
            let m = cache.measure(measurer, s, &node.text, max_width);
            taffy::Size { width: known.width.unwrap_or(m.width), height: known.height.unwrap_or(m.height) }
        }
        NodeKind::Image(src) => {
            let (w, h) = src.size();
            let (w, h) = (w as f32, h as f32);
            match (known.width, known.height) {
                (Some(kw), None) if w > 0.0 => taffy::Size { width: kw, height: kw * h / w },
                (None, Some(kh)) if h > 0.0 => taffy::Size { width: kh * w / h, height: kh },
                _ => taffy::Size { width: w, height: h },
            }
        }
        NodeKind::Div => taffy::Size::ZERO,
    }
}
