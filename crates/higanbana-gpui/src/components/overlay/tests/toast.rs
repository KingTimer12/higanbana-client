//! Toasts: ordem, limite visível, vencimento (com redesenho agendado) e "×".
use super::{center, click, Harness};
use crate::components::{Toast, Toaster, Toasts};
use crate::components::Component;
use crate::prelude::*;
use crate::Tree;
use std::time::Duration;

#[derive(Default)]
struct V {
    toasts: Toasts,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    h.frame(t, |cx| div().size_full().child(Toaster::new("toaster", &v.toasts, |v: &mut V| &mut v.toasts).render(cx)));
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn newest_first_capped_and_expire_in_order() {
    let h = Harness::new();
    let mut q = Toasts::new().max_visible(2);
    let a = q.push_at(Toast::new("a").duration(ms(1000)), h.t0);
    let b = q.push_at(Toast::new("b").duration(ms(1000)), h.t0 + ms(100));
    let c = q.push_at(Toast::new("c").duration(ms(1000)), h.t0 + ms(200));
    assert_eq!(q.visible_ids(h.t0 + ms(200)), vec![c, b], "máximo 2, mais novo primeiro");
    assert_eq!(q.visible_ids(h.t0 + ms(1050)), vec![c, b], "a venceu primeiro");
    assert_eq!(q.visible_ids(h.t0 + ms(1150)), vec![c]);
    q.push_at(Toast::new("d"), h.t0 + ms(1150));
    assert_eq!(q.len(), 2, "vencidos saem da memória no push");
    let _ = a;
}

#[test]
fn toaster_schedules_wake_and_hides_expired() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    v.toasts.push_at(Toast::new("Salvo").description("ok").duration(ms(1000)), h.t0);
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 1);
    assert_eq!(h.wake, Some(h.t0 + ms(850)), "acorda no início do fade de saída");
    h.advance(1001);
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 0);
}

#[test]
fn close_button_dismisses() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    let id = v.toasts.push_at(Toast::new("x"), h.t0);
    frame(&mut h, &mut t, &v);
    let close = super::super::common::sub(super::super::common::sub_i("toaster".into(), "toast", id.0 as usize), "fechar");
    let p = center(&t, close);
    click(&mut t, &mut v, p);
    assert!(v.toasts.is_empty());
}
