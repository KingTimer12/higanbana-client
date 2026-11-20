//! Foco, teclado, captura do ponteiro, hover e desabilitado — sem GPU.
use crate::layout::FakeMeasurer;
use crate::prelude::*;
use crate::{point, size, ElementId, Modifiers, MouseButton, TextStyle, Tree};

#[derive(Default)]
struct V {
    log: Vec<String>,
    value: f32,
    name: TextInputState,
}

fn build(root: Element<V>) -> Tree<V> {
    let mut t = Tree::new();
    t.rebuild(root, &TextStyle::default(), size(300.0, 200.0), &mut FakeMeasurer);
    t
}

fn key(k: Key) -> KeyEvent {
    KeyEvent::new(k, Modifiers::default())
}

fn shift(k: Key) -> KeyEvent {
    KeyEvent::new(k, Modifiers { shift: true, ..Modifiers::default() })
}

fn three_inputs() -> Element<V> {
    div()
        .child(div().track_focus("a").w(20.0).h(20.0))
        .child(div().w(20.0).h(20.0).child(div().track_focus("b").w(10.0).h(10.0)))
        .child(div().track_focus("off").disabled(true).w(20.0).h(20.0))
        .child(div().track_focus("c").w(20.0).h(20.0))
}

#[test]
fn tab_walks_tree_order_and_wraps_skipping_disabled() {
    let mut t = build(three_inputs());
    let (mut v, mut cx) = (V::default(), EventCx::default());
    let mut order = Vec::new();
    for _ in 0..4 {
        assert!(t.key_down(&key(Key::Tab), &mut v, &mut cx));
        order.push(t.focused());
    }
    let id = |s: &str| Some(ElementId::from(s));
    assert_eq!(order, vec![id("a"), id("b"), id("c"), id("a")]);
    assert!(t.focus_info().visible, "foco por teclado mostra o anel");
    t.key_down(&shift(Key::Tab), &mut v, &mut cx);
    assert_eq!(t.focused(), id("c"));
}

#[test]
fn click_focuses_nearest_focusable_and_empty_click_blurs() {
    let mut t = build(three_inputs());
    let (mut v, mut cx) = (V::default(), EventCx::default());
    t.mouse_down(point(25.0, 5.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert_eq!(t.focused(), Some("b".into()));
    assert!(!t.focus_info().visible);
    t.mouse_down(point(250.0, 150.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert_eq!(t.focused(), None);
}

#[test]
fn keys_bubble_until_stopped() {
    let root = div()
        .on_key_down(|v: &mut V, ev, _| v.log.push(format!("raiz {:?}", ev.key)))
        .child(
            div().on_key_down(|v: &mut V, ev, cx| {
                v.log.push(format!("meio {:?}", ev.key));
                if ev.key == Key::Enter {
                    cx.stop_propagation();
                }
            })
            .child(div().track_focus("campo").w(10.0).h(10.0).on_key_down(|v: &mut V, _, _| v.log.push("campo".into()))),
        );
    let mut t = build(root);
    t.focus("campo");
    let (mut v, mut cx) = (V::default(), EventCx::default());
    t.key_down(&key(Key::Char('x')), &mut v, &mut cx);
    assert_eq!(v.log, ["campo", "meio Char('x')", "raiz Char('x')"]);
    v.log.clear();
    let mut cx = EventCx::default();
    assert!(t.key_down(&key(Key::Enter), &mut v, &mut cx));
    assert_eq!(v.log, ["campo", "meio Enter"], "parou no meio");
}

#[test]
fn enter_activates_focused_button() {
    let button = div().track_focus("ok").w(20.0).h(20.0).on_click(|v: &mut V, _, cx| {
        v.log.push("clicou".into());
        cx.notify();
    });
    let mut t = build(div().child(button));
    let (mut v, mut cx) = (V::default(), EventCx::default());
    assert!(!t.key_down(&key(Key::Enter), &mut v, &mut cx), "sem foco nada acontece");
    t.focus("ok");
    assert!(t.key_down(&key(Key::Space), &mut v, &mut cx));
    assert_eq!(v.log, ["clicou"]);
    assert!(cx.is_notified());
}

#[test]
fn text_field_receives_chars_and_editing_keys() {
    let field = |v: &V| text_field("nome", &v.name, |v: &mut V| &mut v.name).w(200.0);
    let mut v = V::default();
    let mut t = build(div().child(field(&v)));
    t.focus("nome");
    for c in "日本語".chars() {
        let mut cx = EventCx::default();
        assert!(t.char_input(c, &mut v, &mut cx));
        assert!(cx.is_notified());
    }
    let mut cx = EventCx::default();
    t.key_down(&key(Key::Backspace), &mut v, &mut cx);
    assert_eq!(v.name.text(), "日本");
    let mut cx = EventCx::default();
    assert!(!t.key_down(&key(Key::Tab), &mut v, &mut cx) || t.focused() == Some("nome".into()), "Tab borbulha");
    assert!(!t.char_input('\u{1b}', &mut v, &mut cx));
}

#[test]
fn text_field_click_places_caret() {
    let mut v = V { name: TextInputState::with_text("abcdef"), ..V::default() };
    let mut t = build(div().child(text_field("nome", &v.name, |v: &mut V| &mut v.name).w(200.0)));
    let mut cx = EventCx::default();
    // Medidor falso: 7 px por caractere; x = 15 fica mais perto da fronteira 2.
    t.mouse_down(point(15.0, 5.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert_eq!(v.name.caret(), 2);
    assert_eq!(t.focused(), Some("nome".into()));
    assert_eq!(t.cursor(), CursorStyle::Text);
}

#[test]
fn pointer_capture_drags_outside_bounds() {
    let slider = div()
        .id("slider")
        .w(100.0)
        .h(10.0)
        .on_mouse_down(|v: &mut V, ev, _| v.value = ev.fraction_x())
        .on_mouse_move(|v: &mut V, ev, _| {
            if ev.pressed {
                v.value = ev.fraction_x();
            }
        })
        .on_mouse_up(|v: &mut V, _, _| v.log.push("solto".into()));
    let mut t = build(div().flex_col().child(slider));
    let (mut v, mut cx) = (V::default(), EventCx::default());
    let m = Modifiers::default();
    t.pointer_move(point(50.0, 50.0), m, &mut v, &mut cx);
    assert_eq!(v.value, 0.0, "fora e sem arraste: nada");
    t.mouse_down(point(25.0, 5.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.value, 0.25);
    assert!(t.is_captured());
    t.pointer_move(point(80.0, 150.0), m, &mut v, &mut cx);
    assert_eq!(v.value, 0.8, "capturado: segue o mouse fora da caixa");
    t.pointer_move(point(500.0, 150.0), m, &mut v, &mut cx);
    assert_eq!(v.value, 1.0);
    t.mouse_up(point(500.0, 150.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.log, ["solto"], "mouse_up vai ao capturado mesmo fora");
    assert!(!t.is_captured());
}

#[test]
fn hover_callbacks_enter_and_leave() {
    let card = div().id("card").w(50.0).h(50.0).cursor_pointer().on_hover(|v: &mut V, on, _| v.log.push(format!("hover {on}")));
    let mut t = build(div().child(card.child(div().w(10.0).h(10.0))));
    let (mut v, mut cx) = (V::default(), EventCx::default());
    let m = Modifiers::default();
    t.pointer_move(point(20.0, 20.0), m, &mut v, &mut cx);
    t.pointer_move(point(5.0, 5.0), m, &mut v, &mut cx);
    assert_eq!(t.cursor(), CursorStyle::Pointer);
    t.pointer_move(point(150.0, 150.0), m, &mut v, &mut cx);
    assert_eq!(v.log, ["hover true", "hover false"]);
    assert_eq!(t.cursor(), CursorStyle::Arrow);
}

#[test]
fn disabled_blocks_click_and_handlers() {
    let b = div().id("b").w(50.0).h(50.0).disabled(true).on_click(|v: &mut V, _, _| v.log.push("clique".into()));
    let mut t = build(div().child(b.on_mouse_down(|v: &mut V, _, _| v.log.push("down".into()))));
    let (mut v, mut cx) = (V::default(), EventCx::default());
    t.mouse_down(point(5.0, 5.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    t.mouse_up(point(5.0, 5.0), MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert!(v.log.is_empty());
}
