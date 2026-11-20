//! Checkbox e Switch: clique, Espaço, rótulo, desabilitado e animação do polegar.
use super::test_util::{build, key, paint, with_cx};
use super::{CheckState, Checkbox, Switch};
use crate::anim::Animator;
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton};
use std::time::Duration;

#[derive(Default)]
struct V {
    on: bool,
    log: Vec<bool>,
}

fn checkbox(v: &V, state: CheckState, disabled: bool) -> Element<V> {
    let mut a = Animator::default();
    with_cx(&mut a, |cx| {
        div().child(
            Checkbox::new(cx, "c", state).label("Aceito").disabled(disabled).on_change(|v: &mut V, b, _| {
                v.on = b;
                v.log.push(b);
            }),
        )
    })
}

fn click(t: &mut crate::Tree<V>, v: &mut V, x: f32, y: f32) {
    let m = Modifiers::default();
    t.mouse_down(point(x, y), MouseButton::Left, m, v, &mut EventCx::default());
    t.mouse_up(point(x, y), MouseButton::Left, m, v, &mut EventCx::default());
}

#[test]
fn checkbox_toggles_by_click_space_and_label() {
    let mut v = V::default();
    let mut t = build(checkbox(&v, false.into(), false));
    click(&mut t, &mut v, 8.0, 8.0);
    assert_eq!(v.log, [true]);
    t = build(checkbox(&v, v.on.into(), false));
    assert!(paint(&t).texts.iter().any(|s| s == "\u{2713}"), "marcado pinta o check");
    t.focus("c");
    assert!(t.key_down(&key(Key::Space), &mut v, &mut EventCx::default()));
    t = build(checkbox(&v, v.on.into(), false));
    click(&mut t, &mut v, 30.0, 8.0); // rótulo
    assert_eq!(v.log, [true, false, true]);
    assert_eq!(t.focused(), Some("c".into()), "rótulo foca a caixa");
    let mut t = build(checkbox(&v, CheckState::Indeterminate, false));
    click(&mut t, &mut v, 8.0, 8.0);
    assert_eq!(v.log.last(), Some(&true), "indeterminado -> marcado");
}

#[test]
fn disabled_checkbox_ignores_input() {
    let mut v = V::default();
    let mut t = build(checkbox(&v, false.into(), true));
    click(&mut t, &mut v, 8.0, 8.0);
    click(&mut t, &mut v, 30.0, 8.0);
    assert!(v.log.is_empty());
}

#[test]
fn switch_toggles_and_thumb_animates_then_idles() {
    let mut a = Animator::default();
    let mut v = V::default();
    let mk = |a: &mut Animator, on: bool, dt: u64, t0: std::time::Instant| {
        with_cx(a, |cx: &mut RenderCx<'_, V>| {
            cx.animator.begin();
            cx.now = t0 + Duration::from_millis(dt);
            let e = div().child(Switch::new(cx, "s", on).on_change(|v: &mut V, b, _| v.log.push(b)));
            cx.animator.end();
            e
        })
    };
    let t0 = std::time::Instant::now();
    let mut t = build(mk(&mut a, false, 0, t0));
    t.focus("s");
    assert!(t.key_down(&key(Key::Enter), &mut v, &mut EventCx::default()));
    assert_eq!(v.log, [true]);
    let thumb_x = |t: &crate::Tree<V>| t.rect(2).origin.x;
    build(mk(&mut a, true, 10, t0));
    assert!(a.is_active(), "começou a animar");
    let mid = build(mk(&mut a, true, 85, t0));
    assert!(a.is_active());
    let x = thumb_x(&mid);
    assert!(x > 1.0 && x < 15.0, "no meio do caminho: {x}");
    let end = build(mk(&mut a, true, 400, t0));
    assert!(!a.is_active(), "terminou: sem frames extras");
    assert_eq!(thumb_x(&end), 15.0);
}
