//! Tooltip: aparece só depois do atraso; sair antes cancela; sair depois esconde.
use super::{center, Harness};
use crate::components::{Tooltip, TooltipState};
use crate::components::Component;
use crate::prelude::*;
use crate::{point, Modifiers, Tree};

#[derive(Default)]
struct V {
    tip: TooltipState,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    h.frame(t, |cx| {
        let tip = Tooltip::new("tip", &v.tip, |v: &mut V| &mut v.tip, div().w(40.0).h(20.0), "Dica").side(Side::Bottom).render(cx);
        div().size_full().p(100.0).child(tip)
    });
}

fn hover(h: &mut Harness, t: &mut Tree<V>, v: &mut V, p: crate::Point) {
    let mut cx = EventCx::default();
    t.pointer_move(p, Modifiers::default(), v, &mut cx);
    h.absorb(&mut cx);
}

#[test]
fn shows_after_delay_and_hides_on_leave() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    let p = center(&t, "tip");
    hover(&mut h, &mut t, &mut v, p);
    assert_eq!(h.timers.len(), 1);
    h.advance(300);
    assert!(h.timers.fire_due(h.now, &mut v).is_none(), "antes do atraso");
    h.advance(250);
    assert!(h.timers.fire_due(h.now, &mut v).unwrap().is_notified());
    assert!(v.tip.visible);
    frame(&mut h, &mut t, &v);
    h.advance(200);
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 1);
    hover(&mut h, &mut t, &mut v, point(700.0, 500.0));
    assert!(!v.tip.visible);
}

#[test]
fn leaving_before_delay_cancels_timer() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    let p = center(&t, "tip");
    hover(&mut h, &mut t, &mut v, p);
    h.advance(100);
    hover(&mut h, &mut t, &mut v, point(700.0, 500.0));
    assert!(h.timers.is_empty(), "timer cancelado");
    assert!(!v.tip.visible);
}
