//! Percorre a árvore posicionada e emite primitivas no `Canvas`.
use super::canvas::{Canvas, Layered};
use crate::color::{rgba, Color};
use crate::geometry::{Corners, Rect};
use crate::gfx::Quad;
use crate::layout::padding_box;
use crate::style::{Display, Overflow, Style};
use crate::tree::node::{children, NodeKind};
use crate::tree::Tree;

/// Cor da barra de rolagem (neutra, funciona nos dois temas).
const SCROLLBAR: Color = rgba(0x8080_8080);
const SCROLLBAR_W: f32 = 4.0;

impl<V> Tree<V> {
    /// Pinta a árvore toda. Devolve quantas camadas extras foram abertas.
    pub fn paint(&self, canvas: &mut dyn Canvas) -> u32 {
        let mut out = Layered::new(canvas);
        if !self.nodes.is_empty() {
            self.paint_node(&mut out, 0, 1.0, None);
        }
        out.layers
    }

    fn paint_node(&self, out: &mut Layered<'_>, i: usize, opacity: f32, text_color: Option<Color>) {
        let n = &self.nodes[i];
        if n.style.display == Display::None {
            return;
        }
        let hovered_style;
        let style: &Style = match &n.hover {
            Some(f) if self.is_hovered(i) => {
                hovered_style = f(n.style.clone());
                &hovered_style
            }
            _ => &n.style,
        };
        let opacity = opacity * style.opacity;
        if opacity <= 0.0 {
            return;
        }
        // Cor de texto vinda de um ancestral em hover sobrepõe a herdada.
        // Um elemento que define a própria cor zera a sobreposição (já está em `n.text`).
        let text_color = match (n.hover.is_some(), style.text.color, n.style.text.color) {
            (true, Some(c), _) => Some(c),
            (_, _, Some(_)) => None,
            _ => text_color,
        };
        let visible = n.clip.intersects(&n.rect);
        if visible && style.has_box() {
            out.quad(&box_quad(style, n.rect, opacity));
        }
        if visible {
            match &n.kind {
                NodeKind::Image(src) => out.image(src, n.content, style.radius, opacity),
                NodeKind::Text(s) => {
                    let color = text_color.unwrap_or(n.text.color).fade(opacity);
                    let max_width = n.text.wrap.then_some(n.content.size.width + 0.5);
                    out.text(s, &n.text, n.content, max_width, color);
                }
                NodeKind::Div => {}
            }
        }
        if n.end as usize == i + 1 {
            return;
        }
        let clips = style.overflow != Overflow::Visible;
        if clips {
            out.push_clip(padding_box(n));
        }
        for c in children(&self.nodes, i) {
            self.paint_node(out, c, opacity, text_color);
        }
        if clips {
            out.pop_clip();
        }
        if n.style.overflow == Overflow::Scroll && n.scroll_max > 0.0 {
            out.quad(&scrollbar(n.rect, n.scroll_max, self.scroll_offset_of(i), opacity));
        }
    }

    fn scroll_offset_of(&self, i: usize) -> f32 {
        self.nodes[i].id.and_then(|id| self.offsets.get(&id).copied()).unwrap_or(0.0)
    }
}

fn box_quad(s: &Style, rect: Rect, opacity: f32) -> Quad {
    let shadow = s.shadow.unwrap_or_default();
    Quad {
        rect,
        fill: s.background.unwrap_or(Color::TRANSPARENT).fade(opacity),
        border: s.border_color.fade(opacity),
        border_width: s.border_width,
        radius: s.radius,
        shadow: shadow.color.fade(opacity),
        shadow_blur: shadow.blur,
    }
}

/// Polegar proporcional à fração visível, na borda direita do contêiner.
fn scrollbar(rect: Rect, max: f32, offset: f32, opacity: f32) -> Quad {
    let h = rect.size.height;
    let thumb = (h * h / (h + max)).max(16.0).min(h);
    let y = rect.origin.y + (h - thumb) * (offset / max);
    Quad {
        rect: Rect::new(rect.right() - SCROLLBAR_W - 2.0, y, SCROLLBAR_W, thumb),
        fill: SCROLLBAR.fade(opacity),
        radius: Corners::all(SCROLLBAR_W / 2.0),
        ..Quad::default()
    }
}
