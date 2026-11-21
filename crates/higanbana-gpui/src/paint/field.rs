//! Pintura de campo de texto: seleção, texto/placeholder e cursor piscando.
use super::canvas::Layered;
use crate::color::Color;
use crate::element::CARET_BLINK_MS;
use crate::geometry::{Corners, Rect};
use crate::gfx::Quad;
use crate::style::TextStyle;
use crate::tree::node::{FieldNode, Node};
use crate::tree::Tree;
use std::time::{Duration, Instant};

const CARET_W: f32 = 1.5;

/// Cursor aceso em `now`? Acende logo após cada edição e alterna a cada meio período.
pub(crate) fn caret_on(epoch: Instant, now: Instant) -> bool {
    (now.saturating_duration_since(epoch).as_millis() / CARET_BLINK_MS as u128) % 2 == 0
}

pub(super) fn paint_field<V>(out: &mut Layered<'_>, n: &Node<V>, f: &FieldNode, focused: bool, now: Instant, color: Color, opacity: f32, text: &TextStyle) {
    // Linha centralizada verticalmente na caixa de conteúdo.
    let line_h = if f.line_h > 0.0 { (f.line_h * n.paint_scale).min(n.content.size.height) } else { n.content.size.height };
    let y = n.content.origin.y + (n.content.size.height - line_h) / 2.0;
    let c = n.content;
    let k = n.paint_scale;
    let x0 = c.origin.x - f.scroll * k;
    let d = &f.data;
    let x_of = |byte: usize| k * f.carets.get(d.text[..byte].chars().count()).copied().unwrap_or(0.0);
    out.push_clip(c.intersect(&n.clip));
    if focused && d.caret != d.anchor {
        let (a, b) = (x_of(d.caret.min(d.anchor)), x_of(d.caret.max(d.anchor)));
        let sel = Rect::new(x0 + a, y, b - a, line_h);
        out.quad(&Quad { rect: sel, fill: d.selection_color.fade(opacity), radius: Corners::all(2.0), ..Quad::default() });
    }
    let bounds = Rect::new(x0, y, (k * f.carets.last().copied().unwrap_or(0.0)).max(1.0), line_h);
    if d.text.is_empty() {
        if !d.placeholder.is_empty() {
            let pc = d.placeholder_color.unwrap_or(color.alpha(color.a * 0.5)).fade(opacity);
            out.text(&d.placeholder, text, bounds, None, pc);
        }
    } else {
        out.text(&d.text, text, bounds, None, color);
    }
    if focused && caret_on(d.epoch, now) {
        let rect = Rect::new((x0 + x_of(d.caret)).min(c.right() - CARET_W), y, CARET_W, line_h);
        out.quad(&Quad { rect, fill: d.caret_color.unwrap_or(color).fade(opacity), ..Quad::default() });
    }
    out.pop_clip();
}

impl<V> Tree<V> {
    /// Quando o cursor do campo focado muda de estado (aceso/apagado): o app só
    /// repinta (sem reconstruir) nesse instante. `None` = nada piscando.
    pub fn next_blink(&self, now: Instant) -> Option<Instant> {
        let i = self.focused_index()?;
        let crate::tree::node::NodeKind::TextField(f) = &self.nodes[i].kind else { return None };
        let half = Duration::from_millis(CARET_BLINK_MS);
        let periods = now.saturating_duration_since(f.data.epoch).as_millis() / half.as_millis() + 1;
        Some(f.data.epoch + half * periods as u32)
    }
}
