//! Teclado: borbulha do focado até a raiz; depois os padrões (Tab, Escape, Enter/Espaço).
use super::key::{Key, KeyEvent};
use crate::cx::EventCx;
use crate::element::MouseEvent;
use crate::geometry::point;
use crate::gfx::{Modifiers, MouseButton};
use crate::tree::node::NO_PARENT;
use crate::tree::Tree;

impl<V> Tree<V> {
    /// Onde começa a propagação: o focado (se recebe entrada), senão a raiz do overlay
    /// modal do topo, senão a raiz principal.
    fn key_target(&self) -> Option<usize> {
        if self.nodes.is_empty() {
            return None;
        }
        self.focused_index().filter(|&i| i >= self.barrier).or(Some(self.barrier))
    }

    /// Tecla pressionada. Devolve `true` se alguém tratou.
    pub fn key_down(&mut self, ev: &KeyEvent, view: &mut V, cx: &mut EventCx) -> bool {
        let mut cur = self.key_target().map_or(NO_PARENT, |i| i as u32);
        while cur != NO_PARENT {
            let n = &self.nodes[cur as usize];
            if let Some(h) = n.interact.as_ref().and_then(|x| x.on_key_down.as_ref()).filter(|_| !n.disabled) {
                h(view, ev, cx);
                if cx.stopped {
                    return true;
                }
            }
            cur = n.parent;
        }
        match ev.key {
            Key::Tab if !ev.mods.ctrl && !ev.mods.alt => self.focus_next(ev.mods.shift),
            Key::Escape => self.dismiss_top(view, cx),
            Key::Enter | Key::Space if !ev.repeat => self.activate_focused(ev.mods, view, cx),
            _ => false,
        }
    }

    /// Caractere digitado: `on_char` do focado (ou da raiz, sem foco) para cima. Controles são ignorados.
    pub fn char_input(&mut self, c: char, view: &mut V, cx: &mut EventCx) -> bool {
        if c.is_control() {
            return false;
        }
        let mut cur = self.key_target().map_or(NO_PARENT, |i| i as u32);
        while cur != NO_PARENT {
            let n = &self.nodes[cur as usize];
            if let Some(h) = n.interact.as_ref().and_then(|x| x.on_char.as_ref()).filter(|_| !n.disabled) {
                h(view, c, cx);
                if cx.stopped {
                    return true;
                }
            }
            cur = n.parent;
        }
        false
    }

    /// Enter/Espaço no focado com `on_click` = clique (botões acessíveis pelo teclado).
    fn activate_focused(&mut self, mods: Modifiers, view: &mut V, cx: &mut EventCx) -> bool {
        let Some(i) = self.focused_index().filter(|&i| !self.nodes[i].disabled) else { return false };
        let n = &self.nodes[i];
        let Some(h) = &n.on_click else { return false };
        let c = point(n.rect.origin.x + n.rect.size.width / 2.0, n.rect.origin.y + n.rect.size.height / 2.0);
        h(view, &MouseEvent { position: c, button: MouseButton::Left, mods }, cx);
        true
    }
}
