//! Popover: abrir/fechar pelo gatilho, Esc, clique fora, alinhamento e seguir a âncora.
use super::{center, click, key, Harness};
use crate::components::{Component, Popover, PopoverAnchor, PopoverState};
use crate::prelude::*;
use crate::{point, Tree};

#[derive(Default)]
struct V {
    p: PopoverState,
    left: f32,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V, anchor: PopoverAnchor) {
    let left = v.left;
    h.frame(t, |cx| {
        let pop = Popover::new("menu", &v.p, |v: &mut V| &mut v.p, div().w(40.0).h(20.0).child(text("Abrir")))
            .anchor(anchor)
            .w(200.0)
            .content(|_| div().id("conteudo").h(50.0).child(text("Itens")))
            .render(cx);
        div().size_full().pl(left).pt(300.0).child(pop)
    });
}

fn open(anchor: PopoverAnchor) -> (Harness, Tree<V>, V) {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V { left: 400.0, ..V::default() });
    frame(&mut h, &mut t, &v, anchor);
    let p = center(&t, "menu");
    click(&mut t, &mut v, p);
    assert!(v.p.open, "clique no gatilho abre");
    frame(&mut h, &mut t, &v, anchor);
    h.advance(300);
    frame(&mut h, &mut t, &v, anchor);
    (h, t, v)
}

#[test]
fn click_trigger_toggles() {
    let (mut h, mut t, mut v) = open(PopoverAnchor::BottomLeft);
    assert_eq!(t.overlay_count(), 1);
    let p = center(&t, "menu");
    click(&mut t, &mut v, p);
    assert!(!v.p.open, "segundo clique fecha");
    frame(&mut h, &mut t, &v, PopoverAnchor::BottomLeft);
    assert_eq!(t.overlay_count(), 1, "saída animando");
    h.advance(500);
    frame(&mut h, &mut t, &v, PopoverAnchor::BottomLeft);
    assert_eq!(t.overlay_count(), 0, "some depois da animação de saída");
}

#[test]
fn escape_closes() {
    let (_, mut t, mut v) = open(PopoverAnchor::BottomLeft);
    let mut cx = EventCx::default();
    t.key_down(&key(Key::Escape), &mut v, &mut cx);
    assert!(!v.p.open);
}

#[test]
fn click_outside_closes() {
    let (_, mut t, mut v) = open(PopoverAnchor::BottomLeft);
    let p = center(&t, "conteudo");
    click(&mut t, &mut v, p);
    assert!(v.p.open, "clique dentro não fecha");
    click(&mut t, &mut v, point(5.0, 5.0));
    assert!(!v.p.open);
}

#[test]
fn bottom_right_opens_above_and_aligns_right_edges() {
    let (_, t, _) = open(PopoverAnchor::BottomRight);
    let trig = t.rect(t.find("menu").unwrap());
    let panel = t.rect(t.find("conteudo").unwrap());
    assert!(panel.bottom() < trig.origin.y, "BottomRight abre acima do gatilho");
    let parent_panel = t.rect(t.find(Popover::<V>::panel_id("menu")).unwrap());
    assert!((parent_panel.right() - trig.right()).abs() < 0.5, "bordas direitas alinhadas");
}

#[test]
fn popover_follows_anchor() {
    let (mut h, mut t, mut v) = open(PopoverAnchor::TopLeft);
    let before = t.rect(t.find("conteudo").unwrap()).origin.x;
    v.left = 100.0;
    frame(&mut h, &mut t, &v, PopoverAnchor::TopLeft);
    let after = t.rect(t.find("conteudo").unwrap()).origin.x;
    assert!((before - after - 300.0).abs() < 0.5, "moveu junto com o gatilho: {before} -> {after}");
}
