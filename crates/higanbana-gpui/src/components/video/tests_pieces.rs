//! Peças do player: AutoHide e VideoSurface.
use super::*;
use crate::components::Component;
use crate::components::overlay::tests::{center, click, Harness};
use crate::gfx::CursorStyle;
use crate::element::ElementId;
use crate::tree::Tree;
use crate::video::Player;
use std::time::{Duration, Instant};

#[test]
fn auto_hide_vence_em_2500ms() {
    let t0 = Instant::now();
    let mut a = AutoHide::new(Duration::from_millis(2500), t0);
    assert_eq!(a.tick(t0 + Duration::from_millis(1000)), Some(Duration::from_millis(1500)));
    assert!(!a.hidden(false));
    assert_eq!(a.tick(t0 + Duration::from_millis(2500)), None);
    assert!(a.hidden(false));
}

#[test]
fn poke_devolve_true_quando_oculto() {
    let t0 = Instant::now();
    let mut a = AutoHide::new(Duration::from_millis(2500), t0);
    assert!(!a.poke(t0));
    a.tick(t0 + Duration::from_secs(3));
    assert!(a.poke(t0 + Duration::from_secs(3)));
    assert!(!a.hidden(false));
}

#[test]
fn hold_segura_visivel() {
    let t0 = Instant::now();
    let mut a = AutoHide::new(Duration::from_millis(2500), t0);
    a.tick(t0 + Duration::from_secs(5));
    assert!(!a.hidden(true));
}

struct S {
    player: Player,
    gate: ClickGate,
    singles: u32,
    doubles: u32,
}

fn s() -> S {
    S { player: Player::detached(), gate: ClickGate::default(), singles: 0, doubles: 0 }
}

fn frame(h: &mut Harness, t: &mut Tree<S>, v: &mut S, hidden: bool) {
    h.frame(t, |cx| {
        VideoSurface::new("sup", &mut v.player, |v: &mut S| &mut v.gate)
            .cursor_hidden(hidden)
            .on_click(|v: &mut S, cx| {
                v.singles += 1;
                cx.notify();
            })
            .on_double_click(|v: &mut S, _| v.doubles += 1)
            .render(cx)
    });
}

#[test]
fn duplo_clique_nao_dispara_simples() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), s());
    frame(&mut h, &mut t, &mut v, false);
    let p = center(&t, "sup");
    let mut a = click(&mut t, &mut v, p);
    h.absorb(&mut a);
    let mut b = click(&mut t, &mut v, p);
    h.absorb(&mut b);
    h.advance(1000);
    h.timers.fire_due(h.now, &mut v);
    assert_eq!((v.singles, v.doubles), (0, 1));
}

#[test]
fn clique_simples_dispara_apos_janela() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), s());
    frame(&mut h, &mut t, &mut v, false);
    let p = center(&t, "sup");
    let mut a = click(&mut t, &mut v, p);
    h.absorb(&mut a);
    assert_eq!(v.singles, 0);
    h.advance(400);
    h.timers.fire_due(h.now, &mut v);
    assert_eq!((v.singles, v.doubles), (1, 0));
}

#[test]
fn cursor_none_quando_oculto() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), s());
    frame(&mut h, &mut t, &mut v, true);
    let n = t.find(ElementId::from("sup")).unwrap();
    assert_eq!(t.nodes[n].style.cursor, Some(CursorStyle::None));
    frame(&mut h, &mut t, &mut v, false);
    let n = t.find(ElementId::from("sup")).unwrap();
    assert_ne!(t.nodes[n].style.cursor, Some(CursorStyle::None));
}

#[test]
fn superficie_informa_a_area_ao_player() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), s());
    frame(&mut h, &mut t, &mut v, false);
    assert_eq!(v.player.state().view_size, Some((800, 600)), "viewport 800x600, escala 1");
}
