//! Rolagem programática: posicionar, consultar o máximo e trazer um filho à vista.
use super::Tree;
use crate::cx::ScrollRequest;
use crate::element::ElementId;
use crate::style::Overflow;

impl<V> Tree<V> {
    /// Executa um pedido de `EventCx`. `true` se a rolagem mudou.
    pub fn apply_scroll(&mut self, r: ScrollRequest) -> bool {
        match r {
            ScrollRequest::To(id, off) => self.scroll_to(id, off),
            ScrollRequest::IntoView(c, k) => self.scroll_into_view(c, k),
        }
    }

    /// `(id, deslocamento, máximo)` de cada contêiner de rolagem com id (para o `RenderCx`).
    pub fn scroll_states(&self) -> Vec<(ElementId, f32, f32)> {
        self.nodes
            .iter()
            .filter(|n| n.style.overflow.is_scroll())
            .filter_map(|n| n.id.map(|id| (id, self.offsets.get(&id).copied().unwrap_or(0.0), n.scroll_max)))
            .collect()
    }

    /// Rolagem máxima do contêiner `id` (0 se não rola ou não existe).
    pub fn scroll_max(&self, id: impl Into<ElementId>) -> f32 {
        self.find(id).map_or(0.0, |i| self.nodes[i].scroll_max)
    }

    /// Fixa o deslocamento do contêiner `id`, preso em `[0, máximo]`. `true` se mudou.
    pub fn scroll_to(&mut self, id: impl Into<ElementId>, offset: f32) -> bool {
        let id = id.into();
        let Some(i) = self.find(id) else { return false };
        let new = offset.clamp(0.0, self.nodes[i].scroll_max);
        let old = self.offsets.insert(id, new).unwrap_or(0.0);
        if old == new {
            return false;
        }
        self.reposition();
        true
    }

    /// Rola o mínimo para o filho `child` (descendente de `container`) ficar inteiro à vista.
    pub fn scroll_into_view(&mut self, container: impl Into<ElementId>, child: impl Into<ElementId>) -> bool {
        let id = container.into();
        let (Some(c), Some(k)) = (self.find(id), self.find(child)) else { return false };
        let (cn, kn) = (&self.nodes[c], &self.nodes[k]);
        let off = self.offsets.get(&id).copied().unwrap_or(0.0);
        let b = cn.style.border_width;
        let (start, len, view) = if cn.style.overflow == Overflow::ScrollX {
            (kn.rect.origin.x - cn.rect.origin.x - b + off, kn.rect.size.width, cn.rect.size.width - 2.0 * b)
        } else {
            (kn.rect.origin.y - cn.rect.origin.y - b + off, kn.rect.size.height, cn.rect.size.height - 2.0 * b)
        };
        let target = if start < off {
            start
        } else if start + len > off + view {
            start + len - view
        } else {
            return false;
        };
        self.scroll_to(id, target)
    }
}
