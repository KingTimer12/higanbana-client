//! object-fit de imagens: contain centraliza sem cortar, cover preenche e recorta.
use super::Canvas;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::style::ObjectFit;
use crate::{Corners, Point, Quad, Rect, TextStyle};

struct V;

#[derive(Default)]
struct Rec {
    images: Vec<Rect>,
    clips: Vec<Rect>,
}

impl Canvas for Rec {
    fn quad(&mut self, _: &Quad) {}
    fn image(&mut self, _: &ImageSource, r: Rect, _: Corners<f32>, _: Color) {
        self.images.push(r);
    }
    fn text(&mut self, _: &str, _: &TextStyle, _: Point, _: Option<f32>, _: Color) {}
    fn push_clip(&mut self, r: Rect) {
        self.clips.push(r);
    }
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {}
}

fn paint(fit: ObjectFit) -> Rec {
    let src = ImageData::from_rgba8(200, 100, vec![0; 200 * 100 * 4]);
    let t = build::<V>(div().child(img(src).w(100.0).h(100.0).object_fit(fit)), 300.0, 300.0);
    let mut r = Rec::default();
    t.paint(&mut r);
    r
}

#[test]
fn fill_stretches() {
    assert_eq!(paint(ObjectFit::Fill).images, vec![Rect::new(0.0, 0.0, 100.0, 100.0)]);
}

#[test]
fn contain_letterboxes() {
    assert_eq!(paint(ObjectFit::Contain).images, vec![Rect::new(0.0, 25.0, 100.0, 50.0)]);
}

#[test]
fn cover_fills_and_clips() {
    let r = paint(ObjectFit::Cover);
    assert_eq!(r.images, vec![Rect::new(-50.0, 0.0, 200.0, 100.0)]);
    assert!(r.clips.contains(&Rect::new(0.0, 0.0, 100.0, 100.0)));
}
