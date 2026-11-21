//! VideoPlayer sem GPU nem mpv (`Player::detached`): tempo, teclas, cliques, auto-ocultar, erro.
use super::*;
use crate::components::Component;
use crate::components::overlay::tests::{center, click, key, Harness};
use crate::gfx::WindowCommand;
use crate::prelude::*;
use crate::tree::node::NodeKind;
use crate::tree::Tree;
use crate::video::{Player, PlayerState, Status};

struct V {
    video: VideoState,
}

fn view(status: Status) -> V {
    let player = Player::detached();
    player.set_state_for_test(PlayerState { status, position: 65.0, duration: 3725.0, ..Default::default() });
    V { video: VideoState::new(player) }
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &mut V) {
    h.frame(t, |cx| VideoPlayer::new("vp", &mut v.video, |v: &mut V| &mut v.video).render(cx));
}

fn texts(t: &Tree<V>) -> Vec<String> {
    t.nodes.iter().filter_map(|n| match &n.kind {
        NodeKind::Text(s) => Some(s.to_string()),
        _ => None,
    }).collect()
}

#[test]
fn controls_show_time_and_progress() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Paused));
    frame(&mut h, &mut t, &mut v);
    assert!(texts(&t).iter().any(|s| s == "1:05 / 1:02:05"), "{:?}", texts(&t));
}

#[test]
fn space_toggles_pause() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Playing));
    frame(&mut h, &mut t, &mut v);
    t.focus(ElementId::from("vp"));
    let mut cx = EventCx::default();
    t.key_down(&key(Key::Space), &mut v, &mut cx);
    t.key_down(&key(Key::Right), &mut v, &mut cx);
    assert_eq!(v.video.player.commands(), vec!["cycle pause", "seek 5.000 relative"]);
}

#[test]
fn double_click_fullscreen() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Playing));
    frame(&mut h, &mut t, &mut v);
    let p = center(&t, surface_id(ElementId::from("vp")));
    let mut a = click(&mut t, &mut v, p);
    h.absorb(&mut a);
    let mut b = click(&mut t, &mut v, p);
    h.absorb(&mut b);
    assert_eq!(b.window_commands, vec![WindowCommand::ToggleFullscreen]);
    h.advance(1000);
    let fired = h.timers.fire_due(h.now, &mut v);
    assert!(fired.is_none(), "clique simples cancelado: sem pausar");
    assert!(v.video.player.commands().is_empty());
}

#[test]
fn single_click_pauses_after_delay() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Playing));
    frame(&mut h, &mut t, &mut v);
    let p = center(&t, surface_id(ElementId::from("vp")));
    let mut a = click(&mut t, &mut v, p);
    h.absorb(&mut a);
    h.advance(400);
    h.timers.fire_due(h.now, &mut v);
    assert_eq!(v.video.player.commands(), vec!["cycle pause"]);
}

#[test]
fn controls_autohide_then_idle() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Playing));
    frame(&mut h, &mut t, &mut v);
    assert!(t.find(controls_id(ElementId::from("vp"))).is_some());
    assert!(h.wake.is_some(), "agenda o sumiço");
    h.advance(3000);
    frame(&mut h, &mut t, &mut v);
    h.advance(300); // fade de saída
    frame(&mut h, &mut t, &mut v);
    assert!(t.find(controls_id(ElementId::from("vp"))).is_none());
    assert!(h.wake.is_none(), "sumiu: nada agendado");
}

#[test]
fn paused_keeps_controls() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Paused));
    h.advance(10_000);
    frame(&mut h, &mut t, &mut v);
    assert!(t.find(controls_id(ElementId::from("vp"))).is_some());
}

#[test]
fn error_overlay() {
    let (mut h, mut t, mut v) = (Harness::new(), Tree::new(), view(Status::Error("arquivo quebrado".into())));
    frame(&mut h, &mut t, &mut v);
    assert!(texts(&t).iter().any(|s| s == "arquivo quebrado"));
}

#[test]
fn time_format() {
    assert_eq!(fmt_time(0.0), "0:00");
    assert_eq!(fmt_time(65.4), "1:05");
    assert_eq!(fmt_time(3725.0), "1:02:05");
}
