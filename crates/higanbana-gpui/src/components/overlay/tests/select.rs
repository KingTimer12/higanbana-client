//! Select: abrir, navegar, escolher (teclado e clique), dispensar e type-ahead.
use super::{center, click, key, Harness};
use crate::components::{Select, SelectState};
use crate::components::Component;
use crate::prelude::*;
use crate::{point, Tree};

#[derive(Default)]
struct V {
    s: SelectState,
    changed: Vec<usize>,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    h.frame(t, |cx| {
        let sel = Select::new("fruta", &v.s, |v: &mut V| &mut v.s)
            .items(["Maçã", "Banana", "Uva"])
            .on_change(|v: &mut V, i, _| v.changed.push(i))
            .render(cx);
        div().size_full().p(10.0).child(sel)
    });
}

#[test]
fn keyboard_open_navigate_choose() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    t.focus("fruta");
    let mut cx = EventCx::default();
    t.key_down(&key(Key::Down), &mut v, &mut cx);
    assert!(v.s.open);
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 1);
    let trig = t.rect(t.find("fruta").unwrap());
    let item = t.rect(t.find(Select::<V>::item_id("fruta", 0)).unwrap());
    assert!(item.origin.y > trig.origin.y + trig.size.height, "lista abaixo do gatilho");
    for k in [Key::Down, Key::Down, Key::Down, Key::Up] {
        t.key_down(&key(k), &mut v, &mut cx);
    }
    assert_eq!(v.s.highlighted, 1, "para no último e volta um");
    t.key_down(&key(Key::Enter), &mut v, &mut cx);
    assert_eq!((v.s.open, v.s.selected, v.changed.clone()), (false, Some(1), vec![1]));
}

#[test]
fn click_item_and_dismiss_outside() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    let p = center(&t, "fruta");
    click(&mut t, &mut v, p);
    assert!(v.s.open);
    frame(&mut h, &mut t, &v);
    let p = center(&t, Select::<V>::item_id("fruta", 2));
    click(&mut t, &mut v, p);
    assert_eq!((v.s.open, v.s.selected), (false, Some(2)));
    frame(&mut h, &mut t, &v);
    v.s.open();
    frame(&mut h, &mut t, &v);
    click(&mut t, &mut v, point(700.0, 500.0));
    assert!(!v.s.open, "clique fora fecha");
    assert_eq!(v.s.selected, Some(2));
}

#[test]
fn escape_closes_and_type_ahead_highlights() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    t.focus("fruta");
    let mut cx = EventCx::default();
    assert!(t.char_input('u', &mut v, &mut cx));
    assert_eq!((v.s.open, v.s.highlighted), (true, 2));
    frame(&mut h, &mut t, &v);
    t.key_down(&key(Key::Escape), &mut v, &mut cx);
    assert!(!v.s.open);
}
