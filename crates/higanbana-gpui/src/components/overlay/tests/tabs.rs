//! Tabs: troca por clique e por setas (foco acompanha).
use super::{center, click, key, Harness};
use crate::components::Tabs;
use crate::prelude::*;
use crate::Tree;

#[derive(Default)]
struct V {
    tab: usize,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    let active = v.tab;
    h.frame(t, |cx| {
        Tabs::new("tabs", active, |v: &mut V, i, _| v.tab = i)
            .tab("Conta", div().id("c0").h(10.0))
            .tab("Senha", div().id("c1").h(10.0))
            .tab("Mais", div().id("c2").h(10.0))
            .render(cx)
    });
}

#[test]
fn only_active_content_and_click_switches() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    assert!(t.find("c0").is_some() && t.find("c1").is_none());
    let p = center(&t, Tabs::<V>::trigger_id("tabs", 1));
    click(&mut t, &mut v, p);
    assert_eq!(v.tab, 1);
    frame(&mut h, &mut t, &v);
    assert!(t.find("c1").is_some() && t.find("c0").is_none());
}

#[test]
fn arrows_move_and_wrap() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    t.focus(Tabs::<V>::trigger_id("tabs", 0));
    let mut cx = EventCx::default();
    assert!(t.key_down(&key(Key::Left), &mut v, &mut cx));
    assert_eq!(v.tab, 2, "esquerda no primeiro dá a volta");
    t.apply_focus(cx.focus.unwrap());
    assert_eq!(t.focused(), Some(Tabs::<V>::trigger_id("tabs", 2)));
    frame(&mut h, &mut t, &v);
    let mut cx = EventCx::default();
    t.key_down(&key(Key::Right), &mut v, &mut cx);
    assert_eq!(v.tab, 0);
}
