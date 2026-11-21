//! Gradiente linear: rampa na CPU desenhada como imagem esticada no retângulo do nó.
use super::gradient::ramp;
use super::Canvas;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::style::{GradientDir, LinearGradient};
use crate::{Corners, Point, Quad, Rect, TextStyle};

struct V;

#[derive(Default)]
struct Rec {
    images: Vec<(u64, Rect)>,
}

impl Canvas for Rec {
    fn quad(&mut self, _: &Quad) {}
    fn image(&mut self, src: &ImageSource, r: Rect, _: Corners<f32>, _: Color) {
        self.images.push((src.id(), r));
    }
    fn text(&mut self, _: &str, _: &TextStyle, _: Point, _: Option<f32>, _: Color) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {}
}

fn scrim() -> LinearGradient {
    LinearGradient::scrim(GradientDir::Down, Color::BLACK.alpha(0.75), Color::BLACK.alpha(0.0))
}

#[test]
fn ramp_endpoints() {
    let src = ramp(&scrim());
    let (w, h) = src.size();
    assert_eq!((w, h), (1, 256), "vertical: 1 coluna");
    let first = src.pending_pixel(0, 0).unwrap();
    let last = src.pending_pixel(0, h - 1).unwrap();
    assert!((first[3] as i32 - 191).abs() <= 1, "alfa inicial 0.75: {first:?}");
    assert_eq!(last[3], 0);
}

#[test]
fn up_reverses_and_right_is_horizontal() {
    let up = ramp(&LinearGradient::scrim(GradientDir::Up, Color::BLACK, Color::TRANSPARENT));
    assert_eq!(up.pending_pixel(0, 255).unwrap()[3], 255, "Up começa embaixo");
    let right = ramp(&LinearGradient::scrim(GradientDir::Right, Color::BLACK, Color::TRANSPARENT));
    assert_eq!(right.size(), (256, 1));
}

#[test]
fn gradient_draws_image_in_rect() {
    let t = build::<V>(div().child(div().w(100.0).h(48.0).bg_gradient(scrim())), 200.0, 200.0);
    let mut r = Rec::default();
    t.paint(&mut r);
    assert_eq!(r.images.len(), 1);
    assert_eq!(r.images[0].1, Rect::new(0.0, 0.0, 100.0, 48.0));
}

#[test]
fn same_gradient_cached() {
    let t = build::<V>(
        div().child(div().w(10.0).h(10.0).bg_gradient(scrim())).child(div().w(10.0).h(10.0).bg_gradient(scrim())),
        200.0,
        200.0,
    );
    let mut r = Rec::default();
    t.paint(&mut r);
    assert_eq!(r.images.len(), 2);
    assert_eq!(r.images[0].0, r.images[1].0);
}
