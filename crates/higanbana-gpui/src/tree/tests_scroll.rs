//! Rolagem horizontal (`scroll_x`) e programática (`scroll_to`, `scroll_into_view`).
use crate::input::SCROLL_STEP;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::{point, scroll_x, ElementId};

struct V;

fn row() -> Element<V> {
    let items = (0..10).map(|i| div().id(("card", i)).w(50.0).h(40.0).flex_shrink_0());
    div().child(scroll_x("linha").w(100.0).h(40.0).children(items))
}

fn list() -> Element<V> {
    let items = (0..10).map(|i| div().id(("item", i)).h(30.0).flex_shrink_0());
    div().child(scroll("lista").w(100.0).h(100.0).children(items))
}

#[test]
fn horizontal_scroll_moves_children() {
    let mut t = build::<V>(row(), 300.0, 300.0);
    t.mouse_move(point(10.0, 10.0));
    assert!(t.scroll_by(-1.0, 0.0, false), "roda horizontal (dx) rola em x");
    assert_eq!(t.scroll_offset("linha"), SCROLL_STEP);
    let first = t.find(("card", 0)).unwrap();
    assert_eq!(t.rect(first).origin.x, -SCROLL_STEP);
    assert_eq!(t.rect(first).origin.y, 0.0);
}

#[test]
fn shift_wheel_scrolls_x() {
    let mut t = build::<V>(row(), 300.0, 300.0);
    t.mouse_move(point(10.0, 10.0));
    assert!(!t.scroll_by(0.0, -1.0, false), "roda vertical sem shift não rola linha horizontal");
    assert!(t.scroll_by(0.0, -1.0, true));
    assert_eq!(t.scroll_offset("linha"), SCROLL_STEP);
}

#[test]
fn scroll_to_clamps() {
    let mut t = build::<V>(row(), 300.0, 300.0);
    t.scroll_to("linha", 9999.0);
    assert_eq!(t.scroll_offset("linha"), 400.0, "10*50 - 100");
    t.scroll_to("linha", -5.0);
    assert_eq!(t.scroll_offset("linha"), 0.0);
    assert_eq!(t.scroll_max("linha"), 400.0);
}

#[test]
fn scroll_into_view_reveals_child() {
    let mut t = build::<V>(list(), 300.0, 300.0);
    t.scroll_into_view("lista", ("item", 5));
    // item 5 ocupa 150..180 no conteúdo; a janela de 100 mostra 80..180.
    assert_eq!(t.scroll_offset("lista"), 80.0);
    let i = t.find(("item", 5)).unwrap();
    assert_eq!(t.rect(i).bottom(), 100.0);
    t.scroll_into_view("lista", ("item", 0));
    assert_eq!(t.scroll_offset("lista"), 0.0);
}

#[test]
fn event_cx_queues_scroll_requests_and_tree_applies() {
    let mut t = build::<V>(row(), 300.0, 300.0);
    let mut cx = EventCx::default();
    cx.scroll_to("linha", 120.0);
    cx.scroll_into_view("linha", ("card", 0));
    for r in cx.scroll_requests.drain(..) {
        t.apply_scroll(r);
    }
    assert_eq!(t.scroll_offset("linha"), 0.0, "scroll_into_view do primeiro volta ao início");
    let states = t.scroll_states();
    assert_eq!(states, vec![(ElementId::from("linha"), 0.0, 400.0)]);
}

#[test]
fn render_cx_pede_rolagem_aplicada_depois_do_layout() {
    let theme = Theme::dark();
    let mut anim = crate::anim::Animator::default();
    let mut cx: RenderCx<'_, V> = RenderCx::new(&theme, BackendKind::D3D11, &[], crate::Stats::default(), &mut anim);
    cx.scroll_to("linha", 120.0);
    let mut t = build::<V>(row(), 300.0, 300.0);
    for r in cx.scroll_requests.drain(..) {
        t.apply_scroll(r);
    }
    assert_eq!(t.scroll_offset("linha"), 120.0);
}

#[test]
fn render_cx_reads_scroll_snapshot() {
    let theme = Theme::dark();
    let mut anim = crate::anim::Animator::default();
    let mut cx: RenderCx<'_, V> = RenderCx::new(&theme, BackendKind::D3D11, &[], crate::Stats::default(), &mut anim);
    cx.scroll = vec![(ElementId::from("linha"), 30.0, 400.0)];
    assert_eq!(cx.scroll_offset("linha"), 30.0);
    assert_eq!(cx.scroll_max("linha"), 400.0);
    assert_eq!(cx.scroll_max("outra"), 0.0);
}
