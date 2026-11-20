//! Pintura sem GPU: grava as chamadas do `Canvas`.
use super::Canvas;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::{point, Corners, Point, Quad, Rect, TextStyle};

#[derive(Debug, PartialEq)]
enum Op {
    Quad(Rect, Color),
    Image,
    Text(String, Color),
    Push(Rect),
    Pop,
    Layer,
}

#[derive(Default)]
struct Rec(Vec<Op>);

impl Canvas for Rec {
    fn quad(&mut self, q: &Quad) {
        self.0.push(Op::Quad(q.rect, q.fill));
    }
    fn image(&mut self, _: &ImageSource, _: Rect, _: Corners<f32>, _: f32) {
        self.0.push(Op::Image);
    }
    fn text(&mut self, t: &str, _: &TextStyle, _: Point, _: Option<f32>, c: Color) {
        self.0.push(Op::Text(t.into(), c));
    }
    fn push_clip(&mut self, r: Rect) {
        self.0.push(Op::Push(r));
    }
    fn pop_clip(&mut self) {
        self.0.push(Op::Pop);
    }
    fn layer(&mut self) {
        self.0.push(Op::Layer);
    }
}

struct V;
const RED: Color = rgb(0xff0000);
const BLUE: Color = rgb(0x0000ff);

#[test]
fn simple_tree_needs_no_layers() {
    let t = build::<V>(div().bg(RED).p(4.0).gap(4.0).child(div().w(10.0).h(10.0).bg(BLUE)).child(text("a")), 100.0, 100.0);
    let mut rec = Rec::default();
    assert_eq!(t.paint(&mut rec), 0);
    assert!(matches!(rec.0[0], Op::Quad(_, c) if c == RED));
    assert!(matches!(rec.0[1], Op::Quad(_, c) if c == BLUE));
    assert!(matches!(rec.0[2], Op::Text(..)));
}

#[test]
fn quad_over_text_opens_layer() {
    let root = div()
        .child(text("texto embaixo"))
        .child(div().absolute().top(0.0).left(0.0).w(50.0).h(20.0).bg(BLUE));
    let t = build::<V>(root, 200.0, 100.0);
    let mut rec = Rec::default();
    assert_eq!(t.paint(&mut rec), 1);
    assert_eq!(rec.0.iter().filter(|o| **o == Op::Layer).count(), 1);
    assert!(matches!(rec.0.last(), Some(Op::Quad(_, c)) if *c == BLUE));
}

#[test]
fn quad_beside_text_shares_layer() {
    let root = div().child(text("ab")).child(div().w(20.0).h(20.0).bg(BLUE));
    let t = build::<V>(root, 200.0, 100.0);
    assert_eq!(t.paint(&mut Rec::default()), 0);
}

#[test]
fn overflow_hidden_clips_children() {
    let t = build::<V>(div().child(div().w(30.0).h(30.0).overflow_hidden().child(div().w(60.0).h(60.0).flex_shrink_0().bg(RED))), 100.0, 100.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert_eq!(rec.0, vec![Op::Push(Rect::new(0.0, 0.0, 30.0, 30.0)), Op::Quad(Rect::new(0.0, 0.0, 60.0, 60.0), RED), Op::Pop]);
}

#[test]
fn hover_style_and_text_color_apply() {
    let button = div().w(50.0).h(20.0).bg(RED).hover(|s| s.bg(BLUE).text_color(Color::WHITE)).child(text("b"));
    let mut t = build::<V>(div().text_color(Color::BLACK).child(button), 100.0, 100.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert_eq!(rec.0[1], Op::Text("b".into(), Color::BLACK));
    t.mouse_move(point(5.0, 5.0));
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert!(matches!(rec.0[0], Op::Quad(_, c) if c == BLUE));
    assert_eq!(rec.0[1], Op::Text("b".into(), Color::WHITE));
}

#[test]
fn opacity_fades_subtree() {
    let t = build::<V>(div().opacity(0.5).child(div().w(10.0).h(10.0).bg(RED)), 100.0, 100.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert!(matches!(rec.0[0], Op::Quad(_, c) if c.a == 0.5));
}
