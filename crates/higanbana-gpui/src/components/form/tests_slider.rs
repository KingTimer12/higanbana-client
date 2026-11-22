//! Slider: clique salta, arraste com captura, limite, passo e teclas.
use super::slider_value::Range;
use super::test_util::{build, key, with_cx};
use super::Slider;
use crate::components::basic::Component;
use crate::anim::Animator;
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton};

#[derive(Default)]
struct V {
    value: f32,
}

fn slider(v: &V, disabled: bool) -> Element<V> {
    let mut a = Animator::default();
    with_cx(&mut a, |cx| {
        div().child(
            Slider::new("sl", v.value)
                .range(0.0, 10.0)
                .step(2.0)
                .width(100.0)
                .disabled(disabled)
                .on_change(|v: &mut V, n, _| v.value = n)
                .render(cx),
        )
    })
}

#[test]
fn range_math_clamps_and_steps() {
    let r = Range { min: 0.0, max: 10.0, step: 2.0 };
    assert_eq!(r.snap(4.9), 4.0);
    assert_eq!(r.snap(5.1), 6.0);
    assert_eq!(r.snap(-3.0), 0.0);
    assert_eq!(r.snap(99.0), 10.0);
    assert_eq!(r.at_fraction(0.33), 4.0);
    assert_eq!(r.key(Key::PageUp, 0.0), Some(10.0));
    assert_eq!(r.key(Key::Char('a'), 0.0), None);
}

#[test]
fn click_jumps_and_drag_follows_with_capture() {
    let mut v = V::default();
    let mut t = build(slider(&v, false));
    let m = Modifiers::default();
    let mut cx = EventCx::default();
    t.mouse_down(point(41.0, 8.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.value, 4.0);
    assert_eq!(cx.focus, Some(crate::cx::FocusRequest::Focus("sl".into())), "clique foca o polegar");
    assert!(t.is_captured());
    t.pointer_move(point(500.0, 150.0), m, &mut v, &mut EventCx::default());
    assert_eq!(v.value, 10.0, "fora da caixa: limitado ao máximo");
    t.mouse_up(point(500.0, 150.0), MouseButton::Left, m, &mut v, &mut EventCx::default());
    assert!(!t.is_captured());
}

#[test]
fn keys_adjust_value() {
    let mut v = V { value: 4.0 };
    let steps = [(Key::Right, 6.0), (Key::Left, 4.0), (Key::End, 10.0), (Key::Home, 0.0), (Key::PageUp, 10.0)];
    for (k, want) in steps {
        let mut t = build(slider(&v, false));
        t.focus("sl");
        assert!(t.key_down(&key(k), &mut v, &mut EventCx::default()));
        assert_eq!(v.value, want, "{k:?}");
    }
}

#[test]
fn disabled_slider_ignores_input() {
    let mut v = V { value: 4.0 };
    let mut t = build(slider(&v, true));
    t.mouse_down(point(90.0, 8.0), MouseButton::Left, Modifiers::default(), &mut v, &mut EventCx::default());
    t.focus("sl");
    t.key_down(&key(Key::End), &mut v, &mut EventCx::default());
    assert_eq!(v.value, 4.0);
}

#[test]
fn slider_do_kit_tem_polegar_branco_sem_borda() {
    let mut a = Animator::default();
    let e = with_cx(&mut a, |cx| div().child(Slider::new("sl", 5.0).range(0.0, 10.0).width(100.0).kit().on_change(|_: &mut V, _, _| {}).render(cx)));
    let t = build(e);
    let n = t.find("sl").unwrap();
    assert_eq!(t.nodes[n].style.background, Some(Color::WHITE));
    assert_eq!(t.nodes[n].style.border_width, 0.0);
}

#[test]
fn soltar_avisa_o_valor_final() {
    #[derive(Default)]
    struct R {
        released: Option<f32>,
    }
    let mut a = Animator::default();
    let el = with_cx(&mut a, |cx| div().child(Slider::new("sl", 0.0).range(0.0, 10.0).step(2.0).width(100.0).on_release(|v: &mut R, n, _| v.released = Some(n)).render(cx)));
    let mut t = build(el);
    let (m, mut v, mut cx) = (Modifiers::default(), R::default(), EventCx::default());
    t.mouse_down(point(41.0, 8.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.released, None, "arrastando: ainda não");
    t.mouse_up(point(81.0, 8.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.released, Some(8.0));
}
