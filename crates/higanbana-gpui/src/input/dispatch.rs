//! Roteamento de mouse: hover, clique (desce+sobe no mesmo elemento) e rolagem.
use crate::cx::EventCx;
use crate::element::MouseEvent;
use crate::geometry::Point;
use crate::gfx::{Modifiers, MouseButton};
use crate::style::Overflow;
use crate::tree::Tree;

/// Pixels lógicos por entalhe da roda.
pub const SCROLL_STEP: f32 = 48.0;

impl<V> Tree<V> {
    /// Atualiza o hover. Devolve `true` se algum estilo de hover mudou (repintar).
    pub fn mouse_move(&mut self, p: Point) -> bool {
        self.mouse = Some(p);
        let before = self.hover_owner(self.hovered);
        self.hovered = self.hit_test(p).map(|i| i as u32);
        before != self.hover_owner(self.hovered)
    }

    /// Mouse saiu da janela.
    pub fn mouse_leave(&mut self) -> bool {
        self.mouse = None;
        let before = self.hover_owner(self.hovered);
        self.hovered = None;
        before.is_some()
    }

    pub fn mouse_down(&mut self, p: Point, button: MouseButton) {
        self.mouse = Some(p);
        self.pressed = if button == MouseButton::Left { self.clickable_at(p).map(|i| i as u32) } else { None };
    }

    /// Solta o botão; chama o handler se o elemento é o mesmo do `mouse_down`.
    /// Devolve `true` se algum handler rodou.
    pub fn mouse_up(&mut self, p: Point, button: MouseButton, mods: Modifiers, view: &mut V, cx: &mut EventCx) -> bool {
        self.mouse = Some(p);
        let pressed = self.pressed.take();
        if button != MouseButton::Left {
            return false;
        }
        let target = self.clickable_at(p);
        match (pressed, target) {
            (Some(a), Some(b)) if a as usize == b => {
                let ev = MouseEvent { position: p, button, mods };
                if let Some(h) = &self.nodes[b].on_click {
                    h(view, &ev, cx);
                }
                true
            }
            _ => false,
        }
    }

    /// Rola o contêiner de rolagem mais próximo sob o mouse.
    /// `delta_y` em entalhes (positivo = roda para cima). Devolve `true` se mudou.
    pub fn scroll(&mut self, delta_y: f32) -> bool {
        let Some(p) = self.mouse else { return false };
        let Some(hit) = self.hit_test(p) else { return false };
        let target = self.nearest(hit, |n| n.style.overflow == Overflow::Scroll && n.id.is_some() && n.scroll_max > 0.0);
        let Some(t) = target else { return false };
        let n = &self.nodes[t];
        let id = n.id.expect("filtrado acima");
        let max = n.scroll_max;
        let off = self.offsets.entry(id).or_insert(0.0);
        let new = (*off - delta_y * SCROLL_STEP).clamp(0.0, max);
        if new == *off {
            return false;
        }
        *off = new;
        self.reposition();
        true
    }

    fn clickable_at(&self, p: Point) -> Option<usize> {
        self.hit_test(p).and_then(|i| self.nearest(i, |n| n.on_click.is_some()))
    }
}
