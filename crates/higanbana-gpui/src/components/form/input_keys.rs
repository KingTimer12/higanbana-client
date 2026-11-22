//! Handlers do Input: edição com aviso de mudança, Enter = enviar e, em senha,
//! cliques/arraste convertidos do texto mascarado e cópia bloqueada.
use super::password::unmask;
use crate::cx::EventCx;
use crate::element::Element;
use crate::input::Key;
use crate::text_input::{InputOutcome, SystemClipboard, TextInputState};
use std::rc::Rc;

/// Callback que recebe o texto atual.
pub(crate) type TextFn<V> = Rc<dyn Fn(&mut V, &str, &mut EventCx)>;

/// Callbacks opcionais do Input.
pub(crate) struct Callbacks<V> {
    pub change: Option<TextFn<V>>,
    pub submit: Option<TextFn<V>>,
}

fn finish<V, L: Fn(&mut V) -> &mut TextInputState>(
    v: &mut V,
    lens: &L,
    out: InputOutcome,
    change: &Option<TextFn<V>>,
    cx: &mut EventCx,
) {
    if !out.handled() {
        return;
    }
    cx.stop_propagation();
    cx.notify();
    if let (InputOutcome::Edited, Some(f)) = (out, change) {
        let text = lens(v).text().to_owned();
        f(v, &text, cx);
    }
}

/// Substitui os handlers padrão do `text_field` pelos do Input.
pub(crate) fn attach<V: 'static, L>(e: Element<V>, lens: L, password: bool, cbs: Callbacks<V>) -> Element<V>
where
    L: Fn(&mut V) -> &mut TextInputState + Clone + 'static,
{
    let (l1, l2, l3, l4) = (lens.clone(), lens.clone(), lens.clone(), lens);
    let (change, change2, submit) = (cbs.change.clone(), cbs.change, cbs.submit);
    let e = e
        .on_key_down(move |v, ev, cx| {
            if ev.key == Key::Enter {
                if let Some(f) = &submit {
                    let text = l1(v).text().to_owned();
                    f(v, &text, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
                return;
            }
            if password && (ev.ctrl('c') || ev.ctrl('x')) {
                cx.stop_propagation();
                return;
            }
            let out = l1(v).handle_key(ev, &mut SystemClipboard);
            finish(v, &l1, out, &change, cx);
        })
        .on_char(move |v, c, cx| {
            let out = l2(v).handle_char(c);
            finish(v, &l2, out, &change2, cx);
        });
    if !password {
        return e;
    }
    e.on_mouse_down(move |v, ev, cx| {
        if let Some(i) = ev.text_index {
            let s = l3(v);
            let real = unmask(&s.text, i);
            s.set_caret(real, ev.mods.shift);
            cx.notify();
        }
    })
    .on_mouse_move(move |v, ev, cx| {
        let Some(i) = ev.text_index.filter(|_| ev.pressed) else { return };
        let s = l4(v);
        let real = unmask(&s.text, i);
        if s.caret() != real {
            s.set_caret(real, true);
            cx.notify();
        }
    })
}
