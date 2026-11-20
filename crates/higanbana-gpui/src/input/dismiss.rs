//! Fechar overlays: clique fora (exceto na âncora) e Escape não tratado.
use crate::cx::EventCx;
use crate::element::Anchor;
use crate::geometry::Point;
use crate::tree::Tree;

impl<V> Tree<V> {
    /// Overlay do topo que sabe se fechar (e recebe entrada: no/acima do modal do topo).
    fn dismissable_top(&self) -> Option<usize> {
        self.overlays.iter().rposition(|o| o.spec.on_dismiss.is_some() && o.root as usize >= self.barrier)
    }

    /// Botão desceu em `p`: se fora do overlay dispensável do topo, chama `on_dismiss`.
    /// Devolve `true` se o clique deve ser engolido (overlay modal).
    pub(crate) fn dismiss_outside(&mut self, p: Point, view: &mut V, cx: &mut EventCx) -> bool {
        let Some(k) = self.dismissable_top() else { return false };
        let o = &self.overlays[k];
        let root = o.root as usize;
        let end = self.nodes[root].end as usize;
        let inside = self.hit_test(p).is_some_and(|i| (root..end).contains(&i));
        let on_anchor = match o.spec.anchor {
            Anchor::Element { id, .. } => self.find(id).is_some_and(|a| self.nodes[a].rect.contains(p)),
            _ => false,
        };
        if inside || on_anchor {
            return false;
        }
        if let Some(h) = &o.spec.on_dismiss {
            h(view, cx);
        }
        o.spec.modal
    }

    /// Escape: fecha o overlay dispensável do topo. `true` se havia um.
    pub(crate) fn dismiss_top(&mut self, view: &mut V, cx: &mut EventCx) -> bool {
        let Some(k) = self.dismissable_top() else { return false };
        if let Some(h) = &self.overlays[k].spec.on_dismiss {
            h(view, cx);
        }
        true
    }
}
