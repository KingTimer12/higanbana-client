//! Seekbar: clique, arraste com captura, marcas, buffer e hover.
use super::*;
use crate::components::overlay::tests::Harness;
use crate::components::Component;
use crate::cx::EventCx;
use crate::element::{div, text, ElementId};
use crate::style::Styled;
use crate::tree::Tree;
use crate::{point, Modifiers, MouseButton};

#[derive(Default)]
struct V {
    bar: SeekbarState,
    seeks: Vec<f64>,
    drags: Vec<bool>,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &mut V, dur: f64, buffered: f64, marks: Vec<f64>) {
    let st = v.bar;
    h.frame(t, |cx| {
        let bar = Seekbar::new("sb", &st, |v: &mut V| &mut v.bar)
            .duration(dur)
            .position(10.0)
            .buffered(buffered)
            .marks(marks)
            .hover_content(|secs, _, _| div().id("dica").child(text(format!("{secs:.0}"))))
            .on_seek(|v: &mut V, s, _| v.seeks.push(s))
            .on_drag(|v: &mut V, d, _| v.drags.push(d))
            .render(cx);
        div().w(200.0).flex_col().child(bar)
    });
}

fn width(t: &Tree<V>, id: ElementId) -> Option<f32> {
    t.find(id).map(|n| t.rect(n).size.width)
}

#[test]
fn fracao_do_clique() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &mut v, 100.0, 0.0, vec![]);
    let m = Modifiers::default();
    let mut cx = EventCx::default();
    t.mouse_down(point(50.0, 10.0), MouseButton::Left, m, &mut v, &mut cx);
    t.mouse_up(point(50.0, 10.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.seeks, vec![25.0]);
    assert_eq!(v.drags, vec![true, false]);
}

#[test]
fn arraste_solto_fora_busca_ultimo_ponto() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &mut v, 100.0, 0.0, vec![]);
    let m = Modifiers::default();
    let mut cx = EventCx::default();
    t.mouse_down(point(100.0, 10.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.bar.drag, Some(0.5));
    t.pointer_move(point(-30.0, 300.0), m, &mut v, &mut cx);
    assert_eq!(v.bar.drag, Some(0.0));
    t.mouse_up(point(-30.0, 300.0), MouseButton::Left, m, &mut v, &mut cx);
    assert_eq!(v.seeks, vec![0.0]);
    assert_eq!(v.bar.drag, None);
}

#[test]
fn sem_duracao_sem_knob() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &mut v, 0.0, 0.0, vec![]);
    assert!(t.find(Seekbar::<V>::knob_id("sb".into())).is_none());
    frame(&mut h, &mut t, &mut v, 100.0, 0.0, vec![]);
    assert!(t.find(Seekbar::<V>::knob_id("sb".into())).is_some());
}

#[test]
fn marca_em_zero_ignorada() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &mut v, 100.0, 0.0, vec![0.0, 30.0, 60.0]);
    assert!(t.find(Seekbar::<V>::mark_id("sb".into(), 0)).is_none());
    assert!(t.find(Seekbar::<V>::mark_id("sb".into(), 1)).is_some());
    assert!(t.find(Seekbar::<V>::mark_id("sb".into(), 2)).is_some());
}

#[test]
fn buffer_limitado_a_1() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &mut v, 100.0, 500.0, vec![]);
    assert_eq!(width(&t, Seekbar::<V>::buffer_id("sb".into())), Some(200.0));
}

#[test]
fn hover_some_ao_sair() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), V::default());
    frame(&mut h, &mut t, &mut v, 100.0, 0.0, vec![]);
    let mut cx = EventCx::default();
    t.pointer_move(point(100.0, 10.0), Modifiers::default(), &mut v, &mut cx);
    assert_eq!(v.bar.hover, Some(100.0));
    frame(&mut h, &mut t, &mut v, 100.0, 0.0, vec![]);
    assert!(t.find("dica").is_some());
    t.pointer_move(point(100.0, 400.0), Modifiers::default(), &mut v, &mut cx);
    assert_eq!(v.bar.hover, None);
}
