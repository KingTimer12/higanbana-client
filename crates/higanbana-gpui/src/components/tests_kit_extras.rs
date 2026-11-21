//! Tag, Link, Breadcrumb, DropdownMenu e TabBar (visuais do kit usados pelo app).
use crate::anim::Animator;
use crate::components::form::test_util::{build, key, paint, with_cx};
use crate::components::{Breadcrumb, Component, DropdownMenu, Link, PopoverState, Tag, TabBar};
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton, Tree};

#[derive(Default)]
struct V {
    log: Vec<String>,
    menu: PopoverState,
}

fn click(t: &mut Tree<V>, v: &mut V, x: f32, y: f32) {
    let m = Modifiers::default();
    t.mouse_down(point(x, y), MouseButton::Left, m, v, &mut EventCx::default());
    t.mouse_up(point(x, y), MouseButton::Left, m, v, &mut EventCx::default());
}

fn with<R>(f: impl FnOnce(&mut RenderCx<'_, V>) -> R) -> R {
    let mut a = Animator::default();
    with_cx(&mut a, f)
}

#[test]
fn tag_secondary_cor_do_tema() {
    let (e, sec) = with(|cx| (div().child(Tag::secondary("Ação").small().render(cx)), cx.theme().secondary));
    let r = paint(&build(e));
    assert!(r.quads.iter().any(|(_, c)| *c == sec));
    assert!(r.texts.iter().any(|s| s == "Ação"));
}

#[test]
fn link_clica() {
    let e = with(|cx| div().child(Link::new("l", "Trailer").on_click(|v: &mut V, _| v.log.push("link".into())).render(cx)));
    let mut t = build(e);
    let mut v = V::default();
    click(&mut t, &mut v, 4.0, 6.0);
    assert_eq!(v.log, ["link"]);
}

#[test]
fn breadcrumb_ultimo_sem_clique() {
    let e = with(|cx| {
        div().flex_col().child(
            Breadcrumb::new("b")
                .link("Início", |v: &mut V, _| v.log.push("inicio".into()))
                .item("Animes")
                .render(cx),
        )
    });
    let mut t = build(e);
    let r = paint(&t);
    assert_eq!(r.texts, ["Início", "Animes"]);
    let mut v = V::default();
    click(&mut t, &mut v, 4.0, 8.0);
    assert_eq!(v.log, ["inicio"]);
    let x = t.rect(t.len() - 1).origin.x + 2.0;
    click(&mut t, &mut v, x, 8.0);
    assert_eq!(v.log, ["inicio"], "último item não navega");
}

fn menu(v: &V) -> Element<V> {
    with(|cx| {
        div().child(
            DropdownMenu::new("m", &v.menu, |v: &mut V| &mut v.menu, text("Ordenar"))
                .item("Nome", true, |v: &mut V, _| v.log.push("nome".into()))
                .item("Ano", false, |v: &mut V, _| v.log.push("ano".into()))
                .render(cx),
        )
    })
}

#[test]
fn dropdown_fecha_ao_escolher() {
    let mut v = V { menu: PopoverState { open: true }, ..V::default() };
    let mut t = build(menu(&v));
    let i = t.find(DropdownMenu::<V>::item_id("m", 1)).expect("item Ano");
    let r = t.rect(i);
    click(&mut t, &mut v, r.origin.x + 4.0, r.origin.y + 4.0);
    assert_eq!(v.log, ["ano"]);
    assert!(!v.menu.open);
}

#[test]
fn dropdown_marca_o_escolhido() {
    let v = V { menu: PopoverState { open: true }, ..V::default() };
    let t = build(menu(&v));
    assert!(t.find(DropdownMenu::<V>::check_id("m", 0)).is_some());
    assert!(t.find(DropdownMenu::<V>::check_id("m", 1)).is_none());
}

#[test]
fn dropdown_esc_fecha() {
    let mut v = V { menu: PopoverState { open: true }, ..V::default() };
    let mut t = build(menu(&v));
    t.focus("m");
    t.key_down(&key(Key::Escape), &mut v, &mut EventCx::default());
    assert!(!v.menu.open);
}

#[test]
fn tabs_underline_ativo_tem_barra() {
    let (e, primary) = with(|cx| {
        let bar = TabBar::new("t", 1, |v: &mut V, i, _| v.log.push(format!("{i}"))).underline().tab("Sobre").tab("Episódios 12").render(cx);
        (div().flex_col().child(bar), cx.theme().primary)
    });
    let mut t = build(e);
    let r = paint(&t);
    assert!(r.quads.iter().any(|(q, c)| *c == primary && q.size.height == 2.0), "indicador de 2 px na cor primária");
    let mut v = V::default();
    click(&mut t, &mut v, 4.0, 10.0);
    assert_eq!(v.log, ["0"]);
}

#[test]
fn tabs_pill_ativo_tem_fundo_primario() {
    let (e, primary) = with(|cx| {
        let bar = TabBar::new("t", 0, |_: &mut V, _, _| {}).pill().tab("Popular").tab("Em alta").render(cx);
        (div().flex_col().child(bar), cx.theme().primary)
    });
    let r = paint(&build(e));
    assert!(r.quads.iter().any(|(_, c)| *c == primary));
}
