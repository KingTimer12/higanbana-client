//! Entrada/saída de hover (`on_hover`) e o cursor do mouse.
use crate::cx::EventCx;
use crate::gfx::CursorStyle;
use crate::tree::node::NO_PARENT;
use crate::tree::{NodeKey, Tree};

impl<V> Tree<V> {
    /// Recalcula quem tem `on_hover` sob o mouse e avisa quem entrou/saiu.
    pub(crate) fn update_hover_owners(&mut self, view: &mut V, cx: &mut EventCx) {
        let mut now: Vec<NodeKey> = Vec::new();
        let mut cur = self.hovered.unwrap_or(NO_PARENT);
        while cur != NO_PARENT {
            let n = &self.nodes[cur as usize];
            if !n.disabled && n.interact.as_ref().is_some_and(|x| x.on_hover.is_some()) {
                now.push(self.key_of(cur as usize));
            }
            cur = n.parent;
        }
        if now == self.hover_owners {
            return;
        }
        let before = std::mem::replace(&mut self.hover_owners, now);
        for (key, entered) in before.iter().filter(|k| !self.hover_owners.contains(k)).map(|k| (*k, false)).collect::<Vec<_>>() {
            self.call_hover(key, entered, view, cx);
        }
        for key in self.hover_owners.clone().into_iter().filter(|k| !before.contains(k)) {
            self.call_hover(key, true, view, cx);
        }
    }

    /// Ids dos elementos com `on_hover` sob o mouse (do mais fundo para fora).
    pub fn hovered_ids(&self) -> Vec<crate::element::ElementId> {
        self.hover_owners.iter().filter_map(|k| self.resolve(*k)).filter_map(|i| self.nodes[i].id).collect()
    }

    fn call_hover(&self, key: NodeKey, entered: bool, view: &mut V, cx: &mut EventCx) {
        let Some(i) = self.resolve(key) else { return };
        if let Some(h) = self.nodes[i].interact.as_ref().and_then(|x| x.on_hover.as_ref()) {
            h(view, entered, cx);
        }
    }

    /// Cursor a mostrar: o do elemento capturado (arraste) ou o mais profundo sob o mouse.
    /// Elemento desabilitado com cursor próprio mostra `NotAllowed`.
    pub fn cursor(&self) -> CursorStyle {
        let start = self.captured.and_then(|k| self.resolve(k)).or(self.hovered.map(|h| h as usize));
        start
            .and_then(|i| self.nearest(i, |n| n.style.cursor.is_some()))
            .and_then(|i| {
                let n = &self.nodes[i];
                if n.disabled { Some(CursorStyle::NotAllowed) } else { n.style.cursor }
            })
            .unwrap_or_default()
    }
}
