//! TitleBar sem GPU: regiões, comandos e glifo de restaurar.
use super::*;
use crate::anim::Animator;
use crate::components::Component;
use crate::gfx::{HitKind, HitRegion, WindowCommand, WindowState};
use crate::layout::tests::build;
use crate::prelude::*;
use crate::tree::Tree;
use crate::{point, Modifiers, MouseButton, Stats};

struct V;

fn bar(state: WindowState) -> Tree<V> {
    let theme = Theme::dark();
    let mut anim = Animator::default();
    let mut cx = RenderCx::new(&theme, BackendKind::D3D11, &[], Stats::default(), &mut anim);
    cx.window = state;
    let e = TitleBar::new("Higanbana").render(&mut cx);
    build(div().w(800.0).child(e), 800.0, 600.0)
}

fn texts(t: &Tree<V>) -> Vec<String> {
    t.nodes.iter().filter_map(|n| match &n.kind {
        crate::tree::node::NodeKind::Text(s) => Some(s.to_string()),
        _ => None,
    }).collect()
}

#[test]
fn title_bar_regions() {
    let t = bar(WindowState::default());
    let mut r: Vec<HitRegion> = Vec::new();
    t.hit_regions(&mut r);
    assert_eq!(r[0].kind, HitKind::Caption);
    assert_eq!(r[0].rect.size.height, TITLE_BAR_HEIGHT);
    let kinds: Vec<HitKind> = r[1..].iter().map(|x| x.kind).collect();
    assert_eq!(kinds, vec![HitKind::Client, HitKind::Maximize, HitKind::Client], "min, max, fechar");
    let close = r[3].rect;
    assert_eq!((close.right(), close.size.width), (800.0, 46.0), "fechar colado na direita");
}

#[test]
fn close_button_sends_command() {
    let mut t = bar(WindowState::default());
    let (mut v, mut cx) = (V, EventCx::default());
    let p = point(800.0 - 10.0, 10.0);
    t.mouse_down(p, MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    t.mouse_up(p, MouseButton::Left, Modifiers::default(), &mut v, &mut cx);
    assert_eq!(cx.window_commands, vec![WindowCommand::Close]);
}

#[test]
fn restore_glyph_when_maximized() {
    assert!(texts(&bar(WindowState::default())).iter().any(|s| s == GLYPH_MAXIMIZE));
    let max = texts(&bar(WindowState { maximized: true, ..Default::default() }));
    assert!(max.iter().any(|s| s == GLYPH_RESTORE) && !max.iter().any(|s| s == GLYPH_MAXIMIZE));
}

#[test]
fn hidden_in_fullscreen() {
    let t = bar(WindowState { fullscreen: true, ..Default::default() });
    let mut r = Vec::new();
    t.hit_regions(&mut r);
    assert!(r.is_empty());
}

fn kit_bar(state: WindowState) -> Tree<V> {
    let theme = Theme::dark();
    let mut anim = Animator::default();
    let mut cx = RenderCx::new(&theme, BackendKind::D3D11, &[], Stats::default(), &mut anim);
    cx.window = state;
    let e = TitleBar::new("").kit().content(div().id("conteudo").w_full().h_full()).render(&mut cx);
    build(div().w(800.0).child(e), 800.0, 600.0)
}

#[test]
fn kit_tem_34px_e_botoes_quadrados() {
    let t = kit_bar(WindowState::default());
    let mut r: Vec<HitRegion> = Vec::new();
    t.hit_regions(&mut r);
    assert_eq!(r[0].rect.size.height, KIT_TITLE_BAR_HEIGHT);
    assert_eq!(KIT_TITLE_BAR_HEIGHT, 34.0);
    let close = r[3].rect;
    assert_eq!((close.right(), close.size.width), (800.0, 34.0), "botões 34×34 como no kit");
    assert!(texts(&t).is_empty(), "sem título nem glifos de fonte: ícones");
}

#[test]
fn kit_conteudo_ocupa_a_barra() {
    let t = kit_bar(WindowState::default());
    let c = t.rect(t.find("conteudo").unwrap());
    assert_eq!(c.origin.x, 12.0, "pl-3");
    assert_eq!(c.right(), 800.0 - 3.0 * 34.0);
}
