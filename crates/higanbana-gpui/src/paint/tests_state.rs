//! Pintura de estados: foco, desabilitado, overlays com backdrop e cursor do campo.
use super::tests::{Op, Rec};
use crate::element::CARET_BLINK_MS;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::Rect;
use std::time::{Duration, Instant};

#[derive(Default)]
struct V {
    s: TextInputState,
}

const RED: Color = rgb(0xff0000);
const BLUE: Color = rgb(0x0000ff);
const DIM: Color = rgba(0x0000_0080);

#[test]
fn focus_and_disabled_styles() {
    let root = div()
        .child(div().track_focus("a").w(10.0).h(10.0).bg(RED).focus(|s| s.bg(BLUE)))
        .child(div().w(10.0).h(10.0).bg(RED).disabled(true).disabled_style(|s| s.opacity(0.5)));
    let mut t = build::<V>(root, 100.0, 100.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert!(matches!(rec.0[0], Op::Quad(_, c) if c == RED));
    assert!(matches!(rec.0[1], Op::Quad(_, c) if c.a == 0.5), "desabilitado a 50%");
    t.focus("a");
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert!(matches!(rec.0[0], Op::Quad(_, c) if c == BLUE));
}

#[test]
fn overlay_paints_in_new_layer_over_backdrop() {
    let dialog = overlay(Anchor::center(), div().w(40.0).h(20.0).bg(BLUE)).modal().backdrop(DIM);
    let t = build::<V>(div().bg(RED).child(dialog), 100.0, 100.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    assert_eq!(
        rec.0,
        vec![
            Op::Quad(Rect::new(0.0, 0.0, 100.0, 100.0), RED),
            Op::Layer,
            Op::Quad(Rect::new(0.0, 0.0, 100.0, 100.0), DIM),
            Op::Quad(Rect::new(30.0, 40.0, 40.0, 20.0), BLUE),
        ]
    );
}

fn field_ops(t: &crate::Tree<V>, at: Instant) -> Vec<Op> {
    let mut rec = Rec::default();
    t.paint_at(&mut rec, at);
    rec.0
}

#[test]
fn field_paints_placeholder_selection_and_blinking_caret() {
    let mut v = V { s: TextInputState::new().placeholder("Buscar") };
    let mk = |v: &V| div().child(text_field("f", &v.s, |v: &mut V| &mut v.s).w(80.0));
    let mut t = build(mk(&v), 200.0, 100.0);
    let ops = field_ops(&t, v.s.epoch());
    assert!(ops.iter().any(|o| matches!(o, Op::Text(s, _) if s == "Buscar")), "{ops:?}");
    assert!(!ops.iter().any(|o| matches!(o, Op::Quad(..))), "sem foco: sem cursor");

    v.s.insert("abcd");
    v.s.move_left(false, true);
    t = build(mk(&v), 200.0, 100.0);
    t.focus("f");
    let quads = |ops: &[Op]| ops.iter().filter_map(|o| if let Op::Quad(r, _) = o { Some(*r) } else { None }).collect::<Vec<_>>();
    let on = quads(&field_ops(&t, v.s.epoch()));
    assert_eq!(on.len(), 2, "seleção + cursor");
    assert_eq!(on[0], Rect::new(21.0, 42.0, 7.0, 16.0), "seleção do último caractere");
    let off = quads(&field_ops(&t, v.s.epoch() + Duration::from_millis(CARET_BLINK_MS + 10)));
    assert_eq!(off.len(), 1, "cursor apagado no meio período seguinte");
    let next = t.next_blink(v.s.epoch() + Duration::from_millis(10)).unwrap();
    assert_eq!(next, v.s.epoch() + Duration::from_millis(CARET_BLINK_MS));
}

#[test]
fn field_scrolls_to_keep_caret_visible() {
    let v = V { s: TextInputState::with_text("0123456789abcdef") };
    let t = build(div().child(text_field("f", &v.s, |v: &mut V| &mut v.s).w(50.0)), 200.0, 100.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    // 16 chars * 7 px = 112; largura 50 -> rola 112 + 2 - 50 = 64.
    assert!(matches!(t.nodes[1].kind, crate::tree::node::NodeKind::TextField(ref f) if f.scroll == 64.0));
}
