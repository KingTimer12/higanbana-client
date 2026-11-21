//! Percorre a árvore posicionada e emite primitivas no `Canvas`.
use super::canvas::{Canvas, Layered};
use crate::color::{rgba, Color};
use crate::geometry::{Corners, Rect};
use crate::gfx::Quad;
use crate::layout::padding_box;
use crate::style::{Display, ObjectFit, Overflow, Style, TextStyle};
use crate::tree::node::{children, NodeKind};
use crate::tree::Tree;
use std::time::Instant;

/// Cor da barra de rolagem (neutra, funciona nos dois temas).
const SCROLLBAR: Color = rgba(0x8080_8080);
const SCROLLBAR_W: f32 = 4.0;

impl<V> Tree<V> {
    /// Pinta a árvore toda. Devolve quantas camadas extras foram abertas.
    pub fn paint(&self, canvas: &mut dyn Canvas) -> u32 {
        self.paint_at(canvas, Instant::now())
    }

    /// Pinta com o relógio dado (piscar do cursor). Overlays vão em camadas por cima.
    pub fn paint_at(&self, canvas: &mut dyn Canvas, now: Instant) -> u32 {
        let mut out = Layered::new(canvas);
        if !self.nodes.is_empty() {
            self.paint_node(&mut out, 0, 1.0, None, now);
        }
        for o in &self.overlays {
            out.new_layer();
            if let Some(c) = o.spec.backdrop {
                let rect = Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height);
                out.quad(&Quad { rect, fill: c, ..Quad::default() });
            }
            self.paint_node(&mut out, o.root as usize, 1.0, None, now);
        }
        out.layers
    }

    /// Estilo de estado (desabilitado, foco, hover) aplicado sobre o base, se houver.
    fn state_style(&self, i: usize) -> Option<Style> {
        let n = &self.nodes[i];
        let x = n.interact.as_deref();
        if n.disabled {
            return x.and_then(|x| x.disabled_style.as_ref()).map(|f| f(n.style.clone()));
        }
        let mut s: Option<Style> = None;
        if let Some(f) = x.and_then(|x| x.focus_style.as_ref()).filter(|_| n.id.is_some() && n.id == self.focused) {
            s = Some(f(n.style.clone()));
        }
        if let Some(f) = n.hover.as_ref().filter(|_| self.is_hovered(i)) {
            s = Some(f(s.unwrap_or_else(|| n.style.clone())));
        }
        s
    }

    fn paint_node(&self, out: &mut Layered<'_>, i: usize, opacity: f32, text_color: Option<Color>, now: Instant) {
        let n = &self.nodes[i];
        if n.style.display == Display::None {
            return;
        }
        let state = self.state_style(i);
        let style: &Style = state.as_ref().unwrap_or(&n.style);
        let opacity = opacity * style.opacity;
        if opacity <= 0.0 {
            return;
        }
        // Cor de texto vinda de um ancestral em hover sobrepõe a herdada.
        // Um elemento que define a própria cor zera a sobreposição (já está em `n.text`).
        let text_color = match (state.is_some(), style.text.color, n.style.text.color) {
            (true, Some(c), _) => Some(c),
            (_, _, Some(_)) => None,
            _ => text_color,
        };
        let visible = n.clip.intersects(&n.rect);
        let k = n.paint_scale;
        if visible && style.has_box() {
            out.quad(&scaled(box_quad(style, n.rect, opacity), k));
        }
        let scaled_text;
        let text_style = if k == 1.0 {
            &n.text
        } else {
            scaled_text = TextStyle { size: n.text.size * k, ..n.text.clone() };
            &scaled_text
        };
        if visible {
            match &n.kind {
                NodeKind::Image(src) => {
                    let dest = fit_rect(n.content, src.size(), style.object_fit);
                    let cover = style.object_fit == ObjectFit::Cover;
                    if cover {
                        out.push_clip(n.content.intersect(&n.clip));
                    }
                    out.image(src, dest, style.radius.map(|r| r * k), opacity);
                    if cover {
                        out.pop_clip();
                    }
                }
                NodeKind::Text(s) => {
                    let color = text_color.unwrap_or(n.text.color).fade(opacity);
                    let max_width = n.text.wrap.then_some(n.content.size.width + 0.5);
                    out.text(s, text_style, n.content, max_width, color);
                }
                NodeKind::TextField(f) => {
                    let color = text_color.unwrap_or(n.text.color).fade(opacity);
                    let focused = n.id.is_some() && n.id == self.focused;
                    super::field::paint_field(out, n, f, focused, now, color, opacity, text_style);
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
            self.paint_node(out, c, opacity, text_color, now);
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
/// Retângulo da imagem `(w, h)` dentro de `content` segundo o encaixe.
fn fit_rect(content: Rect, (w, h): (u32, u32), fit: ObjectFit) -> Rect {
    if fit == ObjectFit::Fill || w == 0 || h == 0 {
        return content;
    }
    let (cw, ch) = (content.size.width, content.size.height);
    let (sx, sy) = (cw / w as f32, ch / h as f32);
    let s = if fit == ObjectFit::Contain { sx.min(sy) } else { sx.max(sy) };
    let (iw, ih) = (w as f32 * s, h as f32 * s);
    Rect::new(content.origin.x + (cw - iw) / 2.0, content.origin.y + (ch - ih) / 2.0, iw, ih)
}

/// Raio, borda e sombra acompanham a escala do transform.
fn scaled(mut q: Quad, k: f32) -> Quad {
    if k != 1.0 {
        q.radius = q.radius.map(|r| r * k);
        q.border_width *= k;
        q.shadow_blur *= k;
    }
    q
}

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
