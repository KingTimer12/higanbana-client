//! translate/scale de subárvore: posições, composição, hit test e escala do texto.
use crate::layout::tests::build;
use crate::paint::Canvas;
use crate::element::ElementId;
use crate::prelude::*;
use crate::{point, Corners, Point, Quad, Rect, TextStyle};

struct V;

fn rect_of(t: &crate::tree::Tree<V>, id: &str) -> Rect {
    let i = t.find(ElementId::from(id)).expect("id");
    t.nodes[i].rect
}

#[test]
fn translate_moves_subtree() {
    let root = div().w(100.0).h(100.0).translate(20.0, 10.0).child(div().id("f").w(10.0).h(10.0));
    let t = build::<V>(root, 300.0, 300.0);
    assert_eq!(rect_of(&t, "f"), Rect::new(20.0, 10.0, 10.0, 10.0));
}

#[test]
fn scale_around_center() {
    let t = build::<V>(div().child(div().id("s").w(100.0).h(100.0).scale(0.5)), 300.0, 300.0);
    assert_eq!(rect_of(&t, "s"), Rect::new(25.0, 25.0, 50.0, 50.0));
}

#[test]
fn nested_composes() {
    let root = div().child(div().id("p").w(100.0).h(100.0).scale(0.5).child(div().id("c").w(20.0).h(20.0).translate(10.0, 0.0)));
    let t = build::<V>(root, 300.0, 300.0);
    // Pai: (25,25) 50x50. Filho (0,0) 20x20 → escalado (25,25) 10x10. O translate do
    // filho vive no espaço do pai (como no CSS): 10 * 0.5 = +5 em x.
    assert_eq!(rect_of(&t, "c"), Rect::new(30.0, 25.0, 10.0, 10.0));
    let i = t.find(ElementId::from("c")).unwrap();
    assert_eq!(t.nodes[i].paint_scale, 0.5);
}

#[test]
fn hit_follows_visual() {
    let root = div().w(300.0).h(50.0).child(div().id("b").w(20.0).h(20.0).translate(200.0, 0.0));
    let t = build::<V>(root, 300.0, 300.0);
    let b = t.find(ElementId::from("b")).unwrap();
    assert_eq!(t.hit_test(point(205.0, 5.0)), Some(b));
    assert_ne!(t.hit_test(point(5.0, 5.0)), Some(b));
}

#[derive(Default)]
struct Sizes(Vec<f32>);

impl Canvas for Sizes {
    fn quad(&mut self, _: &Quad) {}
    fn image(&mut self, _: &ImageSource, _: Rect, _: Corners<f32>, _: f32) {}
    fn text(&mut self, _: &str, s: &TextStyle, _: Point, _: Option<f32>, _: Color) {
        self.0.push(s.size);
    }
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {}
}

#[test]
fn text_size_scaled() {
    let t = build::<V>(div().child(div().text_size(14.0).scale(0.5).child(text("oi"))), 300.0, 300.0);
    let mut c = Sizes::default();
    t.paint(&mut c);
    assert_eq!(c.0, vec![7.0]);
}
