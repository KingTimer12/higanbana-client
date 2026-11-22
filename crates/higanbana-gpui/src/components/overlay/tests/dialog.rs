//! Dialog: Escape, clique no fundo, botão "×", foco ao abrir e Tab preso.
use super::{center, click, key, Harness};
use crate::components::Dialog;
use crate::components::Component;
use crate::prelude::*;
use crate::{point, Tree};

#[derive(Default)]
struct V {
    open: bool,
    closes: u32,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    let open = v.open;
    h.frame(t, |cx| {
        let d = Dialog::new("dlg", open)
            .title("Título")
            .description("Descrição")
            .child(div().track_focus("campo").w(40.0).h(20.0))
            .on_open_change(|v: &mut V, o, _| {
                v.open = o;
                v.closes += 1;
            })
            .render(cx);
        div().size_full().child(div().track_focus("fora").w(10.0).h(10.0)).child(d)
    });
}

fn opened() -> (Harness, Tree<V>, V) {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    let mut cx = EventCx::default();
    v.open = true;
    Dialog::<V>::open(&mut cx, "dlg");
    frame(&mut h, &mut t, &v);
    t.apply_focus(cx.focus.unwrap());
    h.advance(200);
    frame(&mut h, &mut t, &v);
    (h, t, v)
}

#[test]
fn closed_dialog_renders_nothing() {
    let (mut h, mut t, v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 0);
}

#[test]
fn open_focuses_inside_and_traps_tab() {
    let (_h, mut t, mut v) = opened();
    assert_eq!(t.overlay_count(), 1);
    assert_eq!(t.focused(), Some(Dialog::<V>::close_id("dlg")));
    let mut cx = EventCx::default();
    for _ in 0..4 {
        t.key_down(&key(Key::Tab), &mut v, &mut cx);
        assert_ne!(t.focused(), Some("fora".into()), "Tab não sai do modal");
    }
}

#[test]
fn escape_closes_and_exit_animates() {
    let (mut h, mut t, mut v) = opened();
    let mut cx = EventCx::default();
    assert!(t.key_down(&key(Key::Escape), &mut v, &mut cx));
    assert!(!v.open);
    h.advance(50);
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 1, "ainda saindo (fade)");
    h.advance(200);
    frame(&mut h, &mut t, &v);
    assert_eq!(t.overlay_count(), 0);
}

#[test]
fn backdrop_click_and_close_button_close() {
    let (_h, mut t, mut v) = opened();
    click(&mut t, &mut v, point(5.0, 5.0));
    assert_eq!((v.open, v.closes), (false, 1));
    let (_h, mut t, mut v) = opened();
    let p = center(&t, Dialog::<V>::close_id("dlg"));
    click(&mut t, &mut v, p);
    assert_eq!((v.open, v.closes), (false, 1));
}

#[test]
fn kit_abre_perto_do_topo() {
    let (mut h, mut t, v) = (Harness::new(), Tree::<V>::new(), V { open: true, closes: 0 });
    for _ in 0..4 {
        h.frame(&mut t, |cx| div().size_full().child(Dialog::new("dlg", v.open).title("T").kit().render(cx)));
        h.advance(500);
    }
    let i = t.find("dlg").expect("painel");
    assert_eq!(t.nodes[i].rect.origin.y, 60.0, "1/10 da altura (600)");
}
