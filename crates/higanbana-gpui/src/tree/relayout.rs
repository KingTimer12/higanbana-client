//! Layout da árvore principal e dos overlays, medidas e rolagem dos campos de texto.
use super::node::NodeKind;
use super::overlay::place_overlay;
use super::Tree;
use crate::element::Anchor;
use crate::geometry::{point, Size};
use crate::layout::{position, TextMeasurer};

/// Folga entre o cursor e a borda do campo ao rolar horizontalmente.
const FIELD_MARGIN: f32 = 2.0;

impl<V> Tree<V> {
    /// Refaz o layout dos mesmos nós (ex.: redimensionamento).
    pub fn relayout(&mut self, viewport: Size, measurer: &mut dyn TextMeasurer) {
        self.viewport = viewport;
        self.engine.compute(&self.nodes, viewport, measurer);
        self.measure_fields(measurer);
        let end = self.main_end();
        position(&self.engine, &mut self.nodes, 0..end, point(0.0, 0.0), viewport, &mut self.offsets);
        for k in 0..self.overlays.len() {
            let root = self.overlays[k].root as usize;
            let min_width = if self.overlays[k].spec.match_width { self.anchor_rect(k).map(|r| r.size.width) } else { None };
            self.engine.compute_overlay(&self.nodes, root, min_width, measurer);
            self.place(k);
        }
        self.after_position();
    }

    /// Só reposiciona (mudou a rolagem): não chama o taffy.
    pub(crate) fn reposition(&mut self) {
        let end = self.main_end();
        position(&self.engine, &mut self.nodes, 0..end, point(0.0, 0.0), self.viewport, &mut self.offsets);
        for k in 0..self.overlays.len() {
            self.place(k);
        }
        self.after_position();
    }

    fn after_position(&mut self) {
        self.scroll_fields();
        self.hovered = self.mouse.and_then(|p| self.hit_test(p)).map(|i| i as u32);
    }

    /// Caixa do elemento âncora do overlay `k`, se houver.
    pub(crate) fn anchor_rect(&self, k: usize) -> Option<crate::geometry::Rect> {
        match self.overlays[k].spec.anchor {
            Anchor::Element { id, .. } => self.find(id).map(|i| self.nodes[i].rect),
            _ => None,
        }
    }

    /// Posiciona a subárvore do overlay `k` (raiz já calculada pelo taffy).
    fn place(&mut self, k: usize) {
        let root = self.overlays[k].root as usize;
        let end = self.nodes[root].end as usize;
        let l = self.engine.layout(root);
        let size = Size { width: l.size.width, height: l.size.height };
        let spec = &self.overlays[k].spec;
        let origin = place_overlay(&spec.anchor, spec.gap, size, self.anchor_rect(k), self.viewport);
        position(&self.engine, &mut self.nodes, root..end, origin, self.viewport, &mut self.offsets);
    }

    /// x das fronteiras de caractere de cada campo de texto.
    fn measure_fields(&mut self, measurer: &mut dyn TextMeasurer) {
        for n in self.nodes.iter_mut() {
            if let NodeKind::TextField(f) = &mut n.kind {
                measurer.caret_positions(&f.data.text, &n.text, &mut f.carets);
                f.line_h = self.engine.cache.measure(measurer, " ", &n.text, None).height;
            }
        }
    }

    /// Mantém o cursor visível: rola o campo o mínimo necessário (persistido por id).
    fn scroll_fields(&mut self) {
        for n in &mut self.nodes {
            let NodeKind::TextField(f) = &mut n.kind else { continue };
            let width = n.content.size.width;
            let caret_char = f.data.text[..f.data.caret].chars().count();
            let caret_x = f.carets.get(caret_char).copied().unwrap_or(0.0);
            let text_w = f.carets.last().copied().unwrap_or(0.0);
            let mut s = n.id.and_then(|id| self.field_scroll.get(&id).copied()).unwrap_or(0.0);
            if caret_x - s > width - FIELD_MARGIN {
                s = caret_x - width + FIELD_MARGIN;
            }
            if caret_x - s < 0.0 {
                s = caret_x;
            }
            s = s.clamp(0.0, (text_w + FIELD_MARGIN - width).max(0.0));
            f.scroll = s;
            if let Some(id) = n.id {
                self.field_scroll.insert(id, s);
            }
        }
    }
}
