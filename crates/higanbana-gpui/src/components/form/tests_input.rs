//! Input: edição avisa mudança, Enter envia, senha pinta bolinhas, desabilitado ignora.
use super::test_util::{build, key, paint, with_cx};
use super::Input;
use crate::anim::Animator;
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton};

#[derive(Default)]
struct V {
    s: TextInputState,
    log: Vec<String>,
}

fn input(v: &V, password: bool, disabled: bool) -> Element<V> {
    let mut a = Animator::default();
    with_cx(&mut a, |cx| {
        let i = Input::new(cx, "f", &v.s, |v: &mut V| &mut v.s)
            .width(200.0)
            .disabled(disabled)
            .on_change(|v, t, _| v.log.push(format!("change {t}")))
            .on_submit(|v, t, _| v.log.push(format!("submit {t}")));
        div().child(if password { i.password() } else { i })
    })
}

#[test]
fn editing_triggers_change_and_enter_submits() {
    let mut v = V::default();
    let mut t = build(input(&v, false, false));
    t.focus("f");
    for c in "ab".chars() {
        assert!(t.char_input(c, &mut v, &mut EventCx::default()));
    }
    t.key_down(&key(Key::Left), &mut v, &mut EventCx::default());
    t.key_down(&key(Key::Backspace), &mut v, &mut EventCx::default());
    let mut cx = EventCx::default();
    assert!(t.key_down(&key(Key::Enter), &mut v, &mut cx));
    assert!(cx.is_notified());
    assert_eq!(v.log, ["change a", "change ab", "change b", "submit b"], "Left não avisa");
}

#[test]
fn password_paints_bullets_and_maps_clicks() {
    let mut v = V { s: TextInputState::with_text("sécret"), ..V::default() };
    let t = build(input(&v, true, false));
    let r = paint(&t);
    assert!(r.texts.iter().any(|s| s == "••••••"), "{:?}", r.texts);
    assert!(!r.texts.iter().any(|s| s.contains("sécret")));
    let mut t = t;
    // Medidor falso: 7 px por caractere; borda 1 + padding 12 = conteúdo em x 13.
    t.mouse_down(point(13.0 + 14.0, 10.0), MouseButton::Left, Modifiers::default(), &mut v, &mut EventCx::default());
    assert_eq!(v.s.caret(), "sé".len(), "índice mascarado convertido para o real");
}

#[test]
fn disabled_input_ignores_typing() {
    let mut v = V::default();
    let mut t = build(input(&v, false, true));
    t.focus("f");
    assert!(!t.char_input('x', &mut v, &mut EventCx::default()));
    assert!(v.s.is_empty() && v.log.is_empty());
}
