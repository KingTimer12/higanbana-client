//! Layout sem GPU, com o medidor falso (7 px/char em 14 px, linha de 16 px).
use super::FakeMeasurer;
use crate::prelude::*;
use crate::{size, Rect, TextStyle, Tree};

struct V;

pub(crate) fn build<V>(root: Element<V>, w: f32, h: f32) -> Tree<V> {
    let mut t = Tree::new();
    t.rebuild(root, &TextStyle::default(), size(w, h), &mut FakeMeasurer);
    t
}

fn sq() -> Element<V> {
    div().w(20.0).h(20.0)
}

#[test]
fn row_with_gap_and_padding() {
    let t = build::<V>(div().p(10.0).gap(5.0).child(sq()).child(sq()).child(sq()), 200.0, 100.0);
    assert_eq!(t.rect(0), Rect::new(0.0, 0.0, 200.0, 100.0));
    assert_eq!(t.rect(1), Rect::new(10.0, 10.0, 20.0, 20.0));
    assert_eq!(t.rect(2).origin.x, 35.0);
    assert_eq!(t.rect(3).origin.x, 60.0);
}

#[test]
fn column_percent_and_flex_1() {
    let root = div()
        .flex_col()
        .child(div().w(pct(50.0)).h(10.0))
        .child(div().flex_1())
        .child(div().flex_1());
    let t = build::<V>(root, 200.0, 110.0);
    assert_eq!(t.rect(1).size.width, 100.0);
    assert_eq!(t.rect(2), Rect::new(0.0, 10.0, 200.0, 50.0));
    assert_eq!(t.rect(3).origin.y, 60.0);
}

#[test]
fn absolute_inset_and_nesting() {
    let root = div().p(8.0).child(div().w(100.0).h(100.0).p(4.0).child(div().absolute().top(5.0).left(7.0).w(10.0).h(10.0)));
    let t = build::<V>(root, 300.0, 300.0);
    assert_eq!(t.rect(2), Rect::new(15.0, 13.0, 10.0, 10.0));
}

#[test]
fn centered_child() {
    let t = build::<V>(div().items_center().justify_center().child(sq()), 100.0, 60.0);
    assert_eq!(t.rect(1).origin, crate::point(40.0, 20.0));
}

#[test]
fn text_measured_and_wrapped() {
    let t = build::<V>(div().flex_col().items_start().child(text("abcd")), 200.0, 100.0);
    assert_eq!(t.rect(1).size, size(28.0, 16.0));
    let long = "palavra ".repeat(10);
    let t = build::<V>(div().flex_col().child(div().w(70.0).child(text(long))), 200.0, 200.0);
    assert!(t.rect(2).size.height >= 16.0 * 7.0, "{:?}", t.rect(2));
    assert!(t.rect(2).size.width <= 70.0);
}

#[test]
fn text_inherits_size() {
    let t = build::<V>(div().text_size(20.0).flex_col().items_start().child(text("ab")), 200.0, 100.0);
    assert_eq!(t.rect(1).size, size(20.0, 22.0));
}

#[test]
fn hidden_takes_no_space() {
    let t = build::<V>(div().child(sq().hidden()).child(sq()), 100.0, 100.0);
    assert_eq!(t.rect(2).origin.x, 0.0);
}

#[test]
fn relayout_reuses_measurements() {
    let mut t = build::<V>(div().child(text("abc")), 100.0, 100.0);
    let misses = t.engine.cache.misses();
    t.relayout(size(100.0, 80.0), &mut FakeMeasurer);
    assert_eq!(t.engine.cache.misses(), misses);
    assert_eq!(t.rect(0).size, size(100.0, 80.0));
}
