//! Painel de estatísticas no canto inferior direito.
use crate::color::Color;
use crate::geometry::{point, Corners, Rect, Size};
use crate::gfx::{Context, FontSpec, Quad};
use crate::paint::Canvas;
use crate::style::TextStyle;
use crate::theme::Theme;

const W: f32 = 260.0;
const H: f32 = 22.0;

pub(super) fn paint(canvas: &mut dyn Canvas, ctx: &Context, theme: &Theme, viewport: Size) {
    let s = ctx.stats();
    let line = format!(
        "{} · {:.1} ms · GPU {:.1} MB · {} draws",
        ctx.backend().label(),
        s.frame_ms,
        s.gpu_bytes as f64 / 1_048_576.0,
        s.draw_calls
    );
    let rect = Rect::new(viewport.width - W - 8.0, viewport.height - H - 8.0, W, H);
    canvas.layer();
    canvas.quad(&Quad { rect, fill: theme.popover.alpha(0.85), radius: Corners::all(6.0), ..Quad::default() });
    let style = TextStyle { font: FontSpec::new("Consolas", 400, false), size: 12.0, color: theme.popover_foreground, wrap: false };
    canvas.text(&line, &style, point(rect.origin.x + 8.0, rect.origin.y + 4.0), None, Color::fade(theme.popover_foreground, 1.0));
}
