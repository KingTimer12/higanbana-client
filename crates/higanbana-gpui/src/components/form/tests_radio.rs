//! RadioGroup: clique escolhe, setas movem a escolha, horizontal em linha.
use super::test_util::{build, key, with_cx};
use super::RadioGroup;
use crate::anim::Animator;
use crate::components::basic::Component;
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton};

#[derive(Default)]
struct V {
    sel: Option<usize>,
}

fn group(v: &V, horizontal: bool) -> Element<V> {
    let mut a = Animator::default();
    with_cx(&mut a, |cx| {
        let g = RadioGroup::new("legenda", v.sel).items(["Desligada", "Português", "Inglês"]);
        let g = if horizontal { g.horizontal() } else { g };
        let g = g.on_change(|v: &mut V, i, _| v.sel = Some(i)).render(cx);
        div().flex_col().child(g)
    })
}

fn center(t: &crate::Tree<V>, id: crate::ElementId) -> crate::Point {
    let r = t.rect(t.find(id).expect("item"));
    point(r.origin.x + r.size.width / 2.0, r.origin.y + r.size.height / 2.0)
}

#[test]
fn click_selects() {
    let mut v = V::default();
    let mut t = build(group(&v, false));
    let p = center(&t, RadioGroup::<V>::item_id("legenda", 2));
    let m = Modifiers::default();
    t.mouse_down(p, MouseButton::Left, m, &mut v, &mut EventCx::default());
    t.mouse_up(p, MouseButton::Left, m, &mut v, &mut EventCx::default());
    assert_eq!(v.sel, Some(2));
}

#[test]
fn arrows_move_selection() {
    let mut v = V { sel: Some(0) };
    let mut t = build(group(&v, false));
    t.focus("legenda");
    let mut cx = EventCx::default();
    assert!(t.key_down(&key(Key::Down), &mut v, &mut cx));
    assert_eq!(v.sel, Some(1));
    t = build(group(&v, false));
    t.focus("legenda");
    t.key_down(&key(Key::Up), &mut v, &mut cx);
    t = build(group(&v, false));
    t.focus("legenda");
    t.key_down(&key(Key::Up), &mut v, &mut cx);
    assert_eq!(v.sel, Some(0), "não passa do primeiro");
}

#[test]
fn horizontal_lays_out_row() {
    let v = V::default();
    let t = build(group(&v, true));
    let a = t.rect(t.find(RadioGroup::<V>::item_id("legenda", 0)).unwrap());
    let b = t.rect(t.find(RadioGroup::<V>::item_id("legenda", 1)).unwrap());
    assert_eq!(a.origin.y, b.origin.y);
    assert!(b.origin.x > a.right());
    let vt = build(group(&v, false));
    let c = vt.rect(vt.find(RadioGroup::<V>::item_id("legenda", 1)).unwrap());
    assert!(c.origin.y > a.bottom() - 0.5, "vertical empilha");
}
