//! Duplo clique: dois cliques no mesmo elemento, perto no tempo e no espaço.
use std::time::{Duration, Instant};

use crate::layout::FakeMeasurer;
use crate::prelude::*;
use crate::{point, size, EventCx, Modifiers, MouseButton, TextStyle, Tree};

#[derive(Default)]
struct Count {
    clicks: u32,
    doubles: u32,
}

fn tree() -> Tree<Count> {
    let el = div().w(100.0).h(100.0).on_click(|v: &mut Count, _, _| v.clicks += 1).on_double_click(|v: &mut Count, _, _| v.doubles += 1);
    let mut t = Tree::new();
    t.rebuild(div().child(el), &TextStyle::default(), size(200.0, 200.0), &mut FakeMeasurer);
    t
}

fn click_at(t: &mut Tree<Count>, v: &mut Count, x: f32, now: Instant) {
    let mut cx = EventCx::default();
    t.mouse_down(point(x, 10.0), MouseButton::Left, Modifiers::default(), v, &mut cx);
    t.mouse_up_at(point(x, 10.0), MouseButton::Left, Modifiers::default(), v, &mut cx, now);
}

#[test]
fn duplo_clique_dispara() {
    let (mut t, mut v, t0) = (tree(), Count::default(), Instant::now());
    click_at(&mut t, &mut v, 10.0, t0);
    click_at(&mut t, &mut v, 11.0, t0 + Duration::from_millis(100));
    assert_eq!((v.clicks, v.doubles), (2, 1));
    click_at(&mut t, &mut v, 11.0, t0 + Duration::from_millis(200));
    assert_eq!(v.doubles, 1, "terceiro clique não é outro duplo");
}

#[test]
fn cliques_lentos_nao_sao_duplos() {
    let (mut t, mut v, t0) = (tree(), Count::default(), Instant::now());
    click_at(&mut t, &mut v, 10.0, t0);
    click_at(&mut t, &mut v, 10.0, t0 + Duration::from_millis(600));
    assert_eq!(v.doubles, 0);
}

#[test]
fn cliques_longe_nao_sao_duplos() {
    let (mut t, mut v, t0) = (tree(), Count::default(), Instant::now());
    click_at(&mut t, &mut v, 10.0, t0);
    click_at(&mut t, &mut v, 20.0, t0 + Duration::from_millis(100));
    assert_eq!(v.doubles, 0);
}

#[test]
fn caractere_sem_foco_chega_a_raiz() {
    #[derive(Default)]
    struct Typed(Vec<char>);
    let root = div().w(100.0).h(100.0).on_char(|v: &mut Typed, c, _| v.0.push(c));
    let mut t = Tree::new();
    t.rebuild(root, &TextStyle::default(), size(200.0, 200.0), &mut FakeMeasurer);
    let mut v = Typed::default();
    t.char_input('[', &mut v, &mut EventCx::default());
    assert_eq!(v.0, vec!['['], "como as teclas: sem foco, a raiz recebe");
}
