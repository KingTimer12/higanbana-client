//! Foco: clique foca, Tab/Shift+Tab percorrem a ordem da árvore (presos no modal do topo).
use crate::cx::{FocusInfo, FocusRequest};
use crate::element::ElementId;
use crate::style::Display;
use crate::tree::Tree;

impl<V> Tree<V> {
    /// Elemento com foco (pode ter sumido da árvore; volta se reaparecer).
    pub fn focused(&self) -> Option<ElementId> {
        self.focused
    }

    /// Foco e anel de foco (visível só quando veio do teclado).
    pub fn focus_info(&self) -> FocusInfo {
        FocusInfo { focused: self.focused, visible: self.focus_visible }
    }

    pub fn is_focused(&self, id: impl Into<ElementId>) -> bool {
        self.focused == Some(id.into())
    }

    /// Índice do nó focado, se está na árvore atual.
    pub(crate) fn focused_index(&self) -> Option<usize> {
        self.focused.and_then(|id| self.find(id))
    }

    /// Foca `id` (programaticamente: sem anel de foco).
    pub fn focus(&mut self, id: impl Into<ElementId>) {
        self.focused = Some(id.into());
        self.focus_visible = false;
    }

    pub fn blur(&mut self) {
        self.focused = None;
        self.focus_visible = false;
    }

    /// Nó `i` pode receber foco agora?
    fn can_focus(&self, i: usize) -> bool {
        let n = &self.nodes[i];
        n.id.is_some() && !n.disabled && n.interact.as_ref().is_some_and(|x| x.focusable) && self.displayed(i)
    }

    /// Nenhum ancestral (nem ele) está com `display: none`.
    fn displayed(&self, i: usize) -> bool {
        let mut cur = i as u32;
        while cur != crate::tree::node::NO_PARENT {
            let n = &self.nodes[cur as usize];
            if n.style.display == Display::None {
                return false;
            }
            cur = n.parent;
        }
        true
    }

    /// Tab (`reverse = false`) ou Shift+Tab. Dá a volta; `false` se não há focáveis.
    pub fn focus_next(&mut self, reverse: bool) -> bool {
        let range = self.barrier..self.nodes.len();
        let list: Vec<usize> = range.filter(|&i| self.can_focus(i)).collect();
        if list.is_empty() {
            return false;
        }
        let pos = self.focused_index().and_then(|f| list.iter().position(|&i| i == f));
        let next = match (pos, reverse) {
            (None, false) => list[0],
            (None, true) => list[list.len() - 1],
            (Some(p), false) => list[(p + 1) % list.len()],
            (Some(p), true) => list[(p + list.len() - 1) % list.len()],
        };
        self.focused = self.nodes[next].id;
        self.focus_visible = true;
        true
    }

    /// Clique: foca o ancestral focável mais próximo, ou tira o foco.
    pub(crate) fn focus_from_click(&mut self, hit: Option<usize>) {
        match hit.and_then(|i| self.nearest(i, |n| n.interact.as_ref().is_some_and(|x| x.focusable))) {
            Some(i) if self.can_focus(i) => {
                self.focused = self.nodes[i].id;
                self.focus_visible = false;
            }
            Some(_) => {}
            None => self.blur(),
        }
    }

    /// Aplica o pedido de foco feito por um handler.
    pub(crate) fn apply_focus(&mut self, req: FocusRequest) {
        match req {
            FocusRequest::Focus(id) => self.focus(id),
            FocusRequest::Blur => self.blur(),
            FocusRequest::Next => {
                self.focus_next(false);
            }
            FocusRequest::Prev => {
                self.focus_next(true);
            }
        }
    }
}
