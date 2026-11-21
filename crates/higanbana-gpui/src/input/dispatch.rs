//! Roteamento de mouse: hover, clique, desce/sobe/move com captura e rolagem.
use crate::cx::EventCx;
use crate::element::{MouseEvent, PointerEvent};
use crate::geometry::{point, Point};
use crate::gfx::{Modifiers, MouseButton};
use crate::style::Overflow;
use crate::tree::node::NodeKind;
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

    /// Movimento completo: hover, `on_mouse_move` (do capturado, durante arraste)
    /// e `on_hover`. Devolve `true` se precisa repintar.
    pub fn pointer_move(&mut self, p: Point, mods: Modifiers, view: &mut V, cx: &mut EventCx) -> bool {
        let repaint = self.mouse_move(p);
        let target = match self.captured.and_then(|k| self.resolve(k)) {
            Some(c) => Some(c),
            None => self.hit_test(p).and_then(|i| self.nearest(i, |n| !n.disabled && n.interact.as_ref().is_some_and(|x| x.on_mouse_move.is_some()))),
        };
        if let Some(t) = target {
            let ev = self.pointer_event(t, p, MouseButton::Left, mods);
            if let Some(h) = self.nodes[t].interact.as_ref().and_then(|x| x.on_mouse_move.as_ref()) {
                h(view, &ev, cx);
            }
        }
        self.update_hover_owners(view, cx);
        repaint
    }

    pub fn mouse_down(&mut self, p: Point, button: MouseButton, mods: Modifiers, view: &mut V, cx: &mut EventCx) {
        self.mouse = Some(p);
        if self.dismiss_outside(p, view, cx) {
            self.pressed = None;
            return;
        }
        let hit = self.hit_test(p);
        if button == MouseButton::Left {
            self.button_down = true;
            self.focus_from_click(hit);
        }
        self.pressed = if button == MouseButton::Left { self.clickable_at(p).map(|i| i as u32) } else { None };
        let pointer = |n: &crate::tree::node::Node<V>| !n.disabled && n.interact.as_ref().is_some_and(|x| x.has_pointer());
        let Some(t) = hit.and_then(|i| self.nearest(i, pointer)) else { return };
        let ev = self.pointer_event(t, p, button, mods);
        let x = self.nodes[t].interact.as_ref().expect("filtrado acima");
        if button == MouseButton::Left && x.on_mouse_move.is_some() {
            self.captured = Some(self.key_of(t));
        }
        if let Some(h) = &x.on_mouse_down {
            h(view, &ev, cx);
        }
    }

    /// Solta o botão: `on_mouse_up` (do capturado, se houver) e o clique se o elemento
    /// é o mesmo do `mouse_down`. Devolve `true` se o handler de clique rodou.
    pub fn mouse_up(&mut self, p: Point, button: MouseButton, mods: Modifiers, view: &mut V, cx: &mut EventCx) -> bool {
        self.mouse = Some(p);
        let captured = if button == MouseButton::Left { self.captured.take().and_then(|k| self.resolve(k)) } else { None };
        if button == MouseButton::Left {
            self.button_down = false;
        }
        let up = |n: &crate::tree::node::Node<V>| !n.disabled && n.interact.as_ref().is_some_and(|x| x.on_mouse_up.is_some());
        if let Some(t) = captured.or_else(|| self.hit_test(p).and_then(|i| self.nearest(i, up))) {
            let ev = self.pointer_event(t, p, button, mods);
            if let Some(h) = self.nodes[t].interact.as_ref().and_then(|x| x.on_mouse_up.as_ref()) {
                h(view, &ev, cx);
            }
        }
        let pressed = self.pressed.take();
        if let Some(d) = match button {
            MouseButton::Back => Some(crate::gfx::NavDirection::Back),
            MouseButton::Forward => Some(crate::gfx::NavDirection::Forward),
            _ => None,
        } {
            let nav = |n: &crate::tree::node::Node<V>| !n.disabled && n.interact.as_ref().is_some_and(|x| x.on_navigate.is_some());
            if let Some(t) = self.hit_test(p).and_then(|i| self.nearest(i, nav)) {
                if let Some(h) = self.nodes[t].interact.as_ref().and_then(|x| x.on_navigate.as_ref()) {
                    h(view, d, cx);
                }
            }
        }
        if button != MouseButton::Left {
            return false;
        }
        match (pressed, self.clickable_at(p)) {
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
        self.scroll_by(0.0, delta_y, false)
    }

    /// Roda em x e y (entalhes). Com `shift`, a roda vertical rola contêineres horizontais.
    /// Vai para o contêiner mais próximo que rola no eixo com movimento.
    pub fn scroll_by(&mut self, dx: f32, dy: f32, shift: bool) -> bool {
        let Some(p) = self.mouse else { return false };
        let Some(hit) = self.hit_test(p) else { return false };
        let (hx, vy) = if shift { (dx + dy, 0.0) } else { (dx, dy) };
        let target = self.nearest(hit, |n| {
            let delta = match n.style.overflow {
                Overflow::Scroll => vy,
                Overflow::ScrollX => hx,
                _ => 0.0,
            };
            delta != 0.0 && n.id.is_some() && n.scroll_max > 0.0
        });
        let Some(t) = target else { return false };
        let n = &self.nodes[t];
        let id = n.id.expect("filtrado acima");
        let max = n.scroll_max;
        let delta = if n.style.overflow == Overflow::ScrollX { hx } else { vy };
        let off = self.offsets.entry(id).or_insert(0.0);
        let new = (*off - delta * SCROLL_STEP).clamp(0.0, max);
        if new == *off {
            return false;
        }
        *off = new;
        self.reposition();
        true
    }

    /// O ponteiro está capturado por algum elemento (arraste em curso)?
    pub fn is_captured(&self) -> bool {
        self.captured.is_some()
    }

    pub(crate) fn clickable_at(&self, p: Point) -> Option<usize> {
        self.hit_test(p).and_then(|i| self.nearest(i, |n| n.on_click.is_some())).filter(|&i| !self.nodes[i].disabled)
    }

    fn pointer_event(&self, i: usize, p: Point, button: MouseButton, mods: Modifiers) -> PointerEvent {
        let n = &self.nodes[i];
        let local = point(p.x - n.rect.origin.x, p.y - n.rect.origin.y);
        let text_index = match &n.kind {
            NodeKind::TextField(f) => Some(field_index(&f.data.text, &f.carets, (p.x - n.content.origin.x) / n.paint_scale.max(1e-6) + f.scroll)),
            _ => None,
        };
        PointerEvent { position: p, local, bounds: n.rect, button, mods, pressed: self.button_down, text_index }
    }
}

/// Índice de byte da fronteira de caractere mais próxima de `x`.
fn field_index(text: &str, carets: &[f32], x: f32) -> usize {
    let k = carets
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
        .map_or(0, |(k, _)| k);
    text.char_indices().nth(k).map_or(text.len(), |(b, _)| b)
}
