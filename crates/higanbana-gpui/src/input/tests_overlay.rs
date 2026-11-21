//! Overlays: posição, ordem de hit test, modal, clique fora e Escape — sem GPU.
use crate::layout::FakeMeasurer;
use crate::prelude::*;
use crate::{point, size, Modifiers, MouseButton, Rect, TextStyle, Tree};

#[derive(Default)]
struct V {
    open: bool,
    log: Vec<&'static str>,
}

fn build(root: Element<V>) -> Tree<V> {
    let mut t = Tree::new();
    t.rebuild(root, &TextStyle::default(), size(400.0, 300.0), &mut FakeMeasurer);
    t
}

fn down(t: &mut Tree<V>, v: &mut V, x: f32, y: f32) -> EventCx {
    let mut cx = EventCx::default();
    t.mouse_down(point(x, y), MouseButton::Left, Modifiers::default(), v, &mut cx);
    cx
}

/// Botão "gatilho" em (10,10) com dropdown abaixo; fundo clicável em tudo.
fn dropdown(modal: bool) -> Element<V> {
    let mut menu = overlay(Anchor::below("trigger"), div().id("menu").w(100.0).h(60.0).on_click(|v: &mut V, _, _| v.log.push("menu")))
        .gap(4.0)
        .on_dismiss(|v: &mut V, cx| {
            v.open = false;
            v.log.push("fechou");
            cx.notify();
        });
    if modal {
        menu = menu.modal().backdrop(rgba(0x0000_0080));
    }
    div()
        .p(10.0)
        .on_click(|v: &mut V, _, _| v.log.push("fundo"))
        .child(div().id("trigger").w(80.0).h(20.0).child(menu))
}

#[test]
fn overlay_is_placed_below_anchor_and_hit_first() {
    let t = build(dropdown(false));
    assert_eq!(t.overlay_count(), 1);
    let menu = t.find("menu").unwrap();
    assert!(menu > t.find("trigger").unwrap(), "overlay vem depois da árvore principal");
    assert_eq!(t.rect(menu), Rect::new(10.0, 34.0, 100.0, 60.0));
    assert_eq!(t.hit_test(point(50.0, 50.0)), Some(menu), "overlay por cima do fundo");
}

#[test]
fn click_outside_dismisses_and_passes_through_when_not_modal() {
    let mut t = build(dropdown(false));
    let mut v = V::default();
    let cx = down(&mut t, &mut v, 300.0, 250.0);
    assert!(cx.is_notified());
    let mut cx = EventCx::default();
    t.mouse_up(point(300.0, 250.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert_eq!(v.log, ["fechou", "fundo"], "não modal: o clique continua para baixo");
}

#[test]
fn click_on_anchor_or_inside_does_not_dismiss() {
    let mut t = build(dropdown(false));
    let mut v = V::default();
    down(&mut t, &mut v, 20.0, 15.0);
    down(&mut t, &mut v, 50.0, 50.0);
    assert!(v.log.is_empty());
}

#[test]
fn modal_blocks_below_and_swallows_outside_click() {
    let mut t = build(dropdown(true));
    let mut v = V::default();
    assert_eq!(t.hit_test(point(20.0, 15.0)), None, "gatilho abaixo do modal não é atingível");
    let mut cx = down(&mut t, &mut v, 300.0, 250.0);
    t.mouse_up(point(300.0, 250.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert_eq!(v.log, ["fechou"], "modal engole o clique de fora");
}

#[test]
fn escape_dismisses_topmost_and_modal_traps_tab() {
    let dialog = overlay(
        Anchor::center(),
        div().flex_col().child(div().track_focus("d1").w(10.0).h(10.0)).child(div().track_focus("d2").w(10.0).h(10.0)),
    )
    .modal()
    .on_dismiss(|v: &mut V, _| v.log.push("dialog"));
    let root = div().child(div().track_focus("fora").w(10.0).h(10.0)).child(dialog);
    let mut t = build(root);
    let (mut v, mut cx) = (V::default(), EventCx::default());
    let tab = KeyEvent::new(Key::Tab, Modifiers::default());
    let mut seen = Vec::new();
    for _ in 0..3 {
        t.key_down(&tab, &mut v, &mut cx);
        seen.push(t.focused().unwrap());
    }
    assert_eq!(seen, vec!["d1".into(), "d2".into(), "d1".into()], "Tab preso no modal");
    assert!(t.key_down(&KeyEvent::new(Key::Escape, Modifiers::default()), &mut v, &mut cx));
    assert_eq!(v.log, ["dialog"]);
}

#[test]
fn nested_overlays_stack_in_order() {
    let tip = overlay(Anchor::window(0.0, 0.0), div().id("tip").w(30.0).h(30.0)).gap(0.0);
    let dialog = overlay(Anchor::window(0.0, 0.0), div().id("dlg").w(60.0).h(60.0).child(tip)).gap(0.0);
    let t = build(div().child(dialog));
    assert_eq!(t.overlay_count(), 2);
    assert_eq!(t.hit_test(point(5.0, 5.0)), t.find("tip"), "overlay aninhado fica por cima");
    assert_eq!(t.hit_test(point(50.0, 50.0)), t.find("dlg"));
}

#[test]
fn match_width_uses_anchor_width() {
    let menu = overlay(Anchor::below("trigger"), div().id("menu").h(20.0)).match_width();
    let t = build(div().child(div().id("trigger").w(150.0).h(20.0).child(menu)));
    assert_eq!(t.rect(t.find("menu").unwrap()).size.width, 150.0);
}

#[test]
fn overlay_follows_translated_anchor() {
    let root = div()
        .child(div().translate(100.0, 0.0).child(div().id("t").w(40.0).h(20.0)))
        .child(overlay(Anchor::below("t"), div().id("m").w(80.0).h(30.0)).gap(0.0));
    let t = build(root);
    let (a, m) = (t.nodes[t.find("t").unwrap()].rect, t.nodes[t.find("m").unwrap()].rect);
    assert_eq!(a.origin.x, 100.0);
    assert_eq!(m.origin.x, a.origin.x, "menu abre sob o gatilho visível");
    assert_eq!(m.origin.y, a.bottom());
}
