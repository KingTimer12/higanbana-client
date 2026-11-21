//! Hit test, clique, hover e rolagem sem GPU.
use crate::layout::FakeMeasurer;
use crate::prelude::*;
use crate::{point, size, EventCx, Modifiers, MouseButton, TextStyle, Tree};

#[derive(Default)]
struct Counter {
    clicks: u32,
}

fn build(root: Element<Counter>) -> Tree<Counter> {
    let mut t = Tree::new();
    t.rebuild(root, &TextStyle::default(), size(200.0, 200.0), &mut FakeMeasurer);
    t
}

fn click(t: &mut Tree<Counter>, v: &mut Counter, down: (f32, f32), up: (f32, f32)) -> EventCx {
    let mut cx = EventCx::default();
    t.mouse_down(point(down.0, down.1), MouseButton::Left, Modifiers::default(), v, &mut cx);
    t.mouse_up(point(up.0, up.1), MouseButton::Left, Modifiers::default(), v, &mut cx);
    cx
}

#[test]
fn topmost_wins_and_clip_hides() {
    let root = div()
        .child(div().w(100.0).h(100.0).child(div().w(50.0).h(50.0)))
        .child(div().absolute().top(0.0).left(0.0).w(30.0).h(30.0))
        .child(div().absolute().top(150.0).left(0.0).w(40.0).h(40.0).overflow_hidden().child(div().absolute().top(45.0).w(10.0).h(10.0)));
    let t = build(root);
    assert_eq!(t.hit_test(point(10.0, 10.0)), Some(3), "irmão absoluto posterior fica por cima");
    assert_eq!(t.hit_test(point(40.0, 40.0)), Some(2), "filho por cima do pai");
    assert_eq!(t.hit_test(point(80.0, 80.0)), Some(1));
    assert_eq!(t.hit_test(point(5.0, 197.0)), Some(0), "filho fora do clip não é atingido");
}

#[test]
fn click_reaches_handler_and_mutates_view() {
    let button = div().w(50.0).h(50.0).on_click(|v: &mut Counter, _, cx| {
        v.clicks += 1;
        cx.notify();
    });
    let mut t = build(div().child(button.child(text("ok"))));
    let mut v = Counter::default();
    let cx = click(&mut t, &mut v, (5.0, 5.0), (6.0, 6.0));
    assert_eq!(v.clicks, 1, "clique no texto sobe até o botão");
    assert!(cx.is_notified());
    let cx = click(&mut t, &mut v, (5.0, 5.0), (150.0, 150.0));
    assert_eq!(v.clicks, 1, "soltar fora não clica");
    assert!(!cx.is_notified());
}

#[test]
fn hover_dirty_only_when_hover_target_changes() {
    let card = div().w(100.0).h(100.0).hover(|s| s.bg(rgb(0xff0000))).child(div().w(20.0).h(20.0));
    let mut t = build(div().child(card));
    assert!(t.mouse_move(point(50.0, 50.0)), "entrou no card");
    assert!(!t.mouse_move(point(5.0, 5.0)), "filho do mesmo card: nada muda");
    assert!(t.is_hovered(1) && t.is_hovered(2));
    assert!(t.mouse_move(point(150.0, 150.0)), "saiu");
    assert!(!t.mouse_move(point(160.0, 150.0)));
}

fn list() -> Element<Counter> {
    let items = (0..10).map(|i| div().id(("item", i)).h(30.0).flex_shrink_0());
    div().child(scroll("lista").w(100.0).h(100.0).children(items))
}

#[test]
fn scroll_clamps_and_persists_across_rebuild() {
    let mut t = build(list());
    t.mouse_move(point(10.0, 10.0));
    assert!(!t.scroll(1.0), "já está no topo");
    assert!(t.scroll(-1.0));
    assert_eq!(t.scroll_offset("lista"), crate::input::SCROLL_STEP);
    assert_eq!(t.rect(2).origin.y, -crate::input::SCROLL_STEP);
    assert!(t.scroll(-100.0));
    assert_eq!(t.scroll_offset("lista"), 200.0);
    assert!(!t.scroll(-1.0));
    t.rebuild(list(), &TextStyle::default(), size(200.0, 200.0), &mut FakeMeasurer);
    assert_eq!(t.rect(2).origin.y, -200.0, "deslocamento sobrevive à reconstrução");
    let last = t.find(("item", 9)).unwrap();
    assert_eq!(t.rect(last).bottom(), 100.0);
}

#[test]
fn scroll_hidden_items_not_hit() {
    let mut t = build(list());
    t.mouse_move(point(10.0, 10.0));
    t.scroll(-1.0);
    // Rolado 48 px: o item 1 (nó 3) ocupa -18..12.
    assert_eq!(t.hit_test(point(10.0, 5.0)), Some(3));
}
