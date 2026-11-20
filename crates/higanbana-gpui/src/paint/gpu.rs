//! `Canvas` de GPU: escala para pixels físicos e grava na `DrawList`.
use super::canvas::Canvas;
use super::images::ImageCache;
use crate::color::Color;
use crate::element::ImageSource;
use crate::error::Error;
use crate::geometry::{Corners, Point, Rect};
use crate::gfx::{Context, DrawList, Quad};
use crate::style::TextStyle;

const FULL_UV: Rect = Rect::new(0.0, 0.0, 1.0, 1.0);

pub struct GpuCanvas<'l, 'a, 'c> {
    list: &'l mut DrawList<'a>,
    ctx: &'c Context,
    images: &'a ImageCache<'c>,
    scale: f32,
    /// Primeiro erro do frame (o resto do frame continua sendo desenhado).
    pub error: Option<Error>,
}

impl<'l, 'a, 'c> GpuCanvas<'l, 'a, 'c> {
    pub fn new(list: &'l mut DrawList<'a>, ctx: &'c Context, images: &'a ImageCache<'c>, scale: f32) -> Self {
        Self { list, ctx, images, scale, error: None }
    }

    fn record(&mut self, r: crate::Result<()>) {
        if let Err(e) = r {
            self.error.get_or_insert(e);
        }
    }

    fn corners(&self, c: Corners<f32>) -> Corners<f32> {
        let s = self.scale;
        Corners { top_left: c.top_left * s, top_right: c.top_right * s, bottom_right: c.bottom_right * s, bottom_left: c.bottom_left * s }
    }
}

impl<'a, 'c> Canvas for GpuCanvas<'_, 'a, 'c> {
    fn quad(&mut self, q: &Quad) {
        let s = self.scale;
        let scaled = Quad {
            rect: q.rect.scale(s),
            radius: self.corners(q.radius),
            border_width: q.border_width * s,
            shadow_blur: q.shadow_blur * s,
            ..*q
        };
        let r = self.list.quad(&scaled);
        self.record(r);
    }

    fn image(&mut self, src: &ImageSource, rect: Rect, radius: Corners<f32>, opacity: f32) {
        let images: &'a ImageCache<'c> = self.images;
        let Some(img) = images.get(src) else { return };
        let tint = Color::WHITE.fade(opacity);
        let (dest, radius) = (rect.scale(self.scale), self.corners(radius));
        let r = self.list.image(img, dest, FULL_UV, tint, radius);
        self.record(r);
    }

    fn text(&mut self, text: &str, style: &TextStyle, origin: Point, max_width: Option<f32>, color: Color) {
        let s = self.scale;
        let r = self.ctx.font(&style.font).and_then(|font| {
            let mw = max_width.map_or(0.0, |w| w * s);
            self.list.text(font, style.size * s, origin.x * s, origin.y * s, mw, color, text)
        });
        self.record(r);
    }

    fn push_clip(&mut self, r: Rect) {
        let r = self.list.push_clip(r.scale(self.scale));
        self.record(r);
    }

    fn pop_clip(&mut self) {
        self.list.pop_clip();
    }

    fn layer(&mut self) {
        let r = self.list.layer();
        self.record(r);
    }
}
