//! Medidor de texto real (DirectWrite via core), convertendo lógico <-> físico.
use crate::geometry::Size;
use crate::gfx::Context;
use crate::layout::TextMeasurer;
use crate::style::TextStyle;

pub struct GpuMeasurer<'c> {
    pub ctx: &'c Context,
    pub scale: f32,
}

impl TextMeasurer for GpuMeasurer<'_> {
    fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> Size {
        let s = self.scale;
        let result = self.ctx.font(&style.font).and_then(|font| {
            self.ctx.measure_text(font, style.size * s, text, max_width.map_or(0.0, |w| (w * s).max(1.0)))
        });
        match result {
            Ok(m) => Size { width: m.width / s, height: m.height / s },
            Err(e) => {
                eprintln!("higanbana-gpui: medir texto falhou: {e}");
                Size::default()
            }
        }
    }

    /// Um layout DirectWrite só para todas as fronteiras.
    fn caret_positions(&mut self, text: &str, style: &TextStyle, out: &mut Vec<f32>) {
        let s = self.scale;
        let r = self.ctx.font(&style.font).and_then(|font| self.ctx.caret_positions(font, style.size * s, text, out));
        match r {
            Ok(()) => out.iter_mut().for_each(|x| *x /= s),
            Err(e) => {
                eprintln!("higanbana-gpui: posições do cursor falharam: {e}");
                out.clear();
                out.resize(text.chars().count() + 1, 0.0);
            }
        }
    }
}
