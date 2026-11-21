//! Sidebar: larguras, item ativo, modo recolhido e cliques.
use super::*;
use crate::anim::Animator;
use crate::components::basic::Component;
use crate::components::form::test_util::{build, paint, with_cx};
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton};

#[derive(Default)]
struct V {
    side: SidebarState,
    clicks: Vec<&'static str>,
    toggled: u32,
}

fn bar(v: &V) -> Element<V> {
    let mut a = Animator::default();
    with_cx(&mut a, |cx| {
        let toggle = SidebarToggle::new("recolher", v.side.collapsed).on_click(|v: &mut V, _, _| v.toggled += 1).render(cx);
        let group = SidebarGroup::new("Acervo")
            .item(SidebarItem::new("inicio", "Início").icon("house").active(true).on_click(|v: &mut V, _, _| v.clicks.push("inicio")))
            .item(SidebarItem::new("animes", "Animes").icon("tv").on_click(|v: &mut V, _, _| v.clicks.push("animes")));
        let s = Sidebar::new("nav", &v.side, |v: &mut V| &mut v.side).header(toggle).group(group).render(cx);
        div().h(600.0).child(s)
    })
}

fn tree(v: &V) -> crate::Tree<V> {
    let mut t = build(bar(v));
    t.rebuild(bar(v), &crate::TextStyle::default(), crate::size(800.0, 600.0), &mut crate::layout::FakeMeasurer);
    t
}

fn click(t: &mut crate::Tree<V>, v: &mut V, id: &str) {
    let r = t.rect(t.find(id).expect("elemento"));
    let p = point(r.origin.x + r.size.width / 2.0, r.origin.y + r.size.height / 2.0);
    let m = Modifiers::default();
    t.mouse_down(p, MouseButton::Left, m, v, &mut EventCx::default());
    t.mouse_up(p, MouseButton::Left, m, v, &mut EventCx::default());
}

#[test]
fn largura_expandida_e_recolhida() {
    let mut v = V::default();
    let t = tree(&v);
    assert_eq!(t.rect(t.find("nav").unwrap()).size.width, SIDEBAR_WIDTH);
    assert_eq!(SIDEBAR_WIDTH, 255.0);
    v.side.collapsed = true;
    // Sem animação em curso no primeiro frame: vai direto ao alvo.
    let t = tree(&v);
    assert_eq!(t.rect(t.find("nav").unwrap()).size.width, SIDEBAR_COLLAPSED_WIDTH);
    assert_eq!(SIDEBAR_COLLAPSED_WIDTH, 48.0);
}

#[test]
fn item_ativo_usa_cor_accent() {
    let v = V::default();
    let t = tree(&v);
    let accent = Theme::dark().sidebar_accent;
    let r = t.rect(t.find("inicio").unwrap());
    let rec = paint(&t);
    assert!(rec.quads.iter().any(|(q, c)| *q == r && *c == accent), "ativo pinta sidebar_accent");
    let r2 = t.rect(t.find("animes").unwrap());
    assert!(!rec.quads.iter().any(|(q, c)| *q == r2 && *c == accent), "inativo sem fundo");
}

#[test]
fn recolhida_esconde_rotulos_mas_mantem_icones() {
    let mut v = V::default();
    let texts = paint(&tree(&v)).texts;
    assert!(texts.iter().any(|s| s == "Início") && texts.iter().any(|s| s == "Acervo"));
    v.side.collapsed = true;
    let t = tree(&v);
    let texts = paint(&t).texts;
    assert!(!texts.iter().any(|s| s == "Início" || s == "Acervo"), "sem rótulos: {texts:?}");
    let r = t.rect(t.find("inicio").unwrap());
    assert!(r.size.width > 0.0 && r.size.width <= SIDEBAR_COLLAPSED_WIDTH, "item vira só ícone");
}

#[test]
fn recolhida_item_ativo_continua_destacado() {
    let mut v = V::default();
    v.side.collapsed = true;
    let t = tree(&v);
    let r = t.rect(t.find("inicio").unwrap());
    assert!(paint(&t).quads.iter().any(|(q, c)| *q == r && *c == Theme::dark().sidebar_accent));
}

#[test]
fn clique_no_item_chama_handler() {
    let mut v = V::default();
    let mut t = tree(&v);
    click(&mut t, &mut v, "animes");
    assert_eq!(v.clicks, vec!["animes"]);
}

#[test]
fn toggle_chama_handler() {
    let mut v = V::default();
    let mut t = tree(&v);
    click(&mut t, &mut v, "recolher");
    assert_eq!(v.toggled, 1);
}
