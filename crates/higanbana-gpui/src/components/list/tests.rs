//! Lista virtual (só constrói o que aparece) e `on_visible`.
use super::{item_offset, visible_range, VirtualList};
use crate::anim::Animator;
use crate::components::form::test_util::{build, with_cx};
use crate::cx::EventCx;
use crate::prelude::*;
use crate::element::ElementId;
use std::sync::Arc;

#[derive(Default)]
struct V {
    vistos: Vec<&'static str>,
}

#[test]
fn vazia_nao_constroi_nada() {
    let r = visible_range(&[], 0.0, 0.0, 400.0);
    assert_eq!((r.start, r.end, r.total), (0, 0, 0.0));
}

#[test]
fn so_constroi_visiveis() {
    let h = vec![40.0; 1000];
    let r = visible_range(&h, 0.0, 2000.0, 400.0);
    // Visíveis: 50..60; com uma tela de folga para cada lado: 40..70.
    assert_eq!((r.start, r.end), (40, 70));
    assert_eq!(r.top, 1600.0);
    assert_eq!(r.total, 40_000.0);
}

#[test]
fn alturas_variaveis_offset_certo() {
    let h = [10.0, 20.0, 30.0, 40.0];
    assert_eq!(item_offset(&h, 2.0, 0), 0.0);
    assert_eq!(item_offset(&h, 2.0, 2), 34.0);
    let r = visible_range(&h, 2.0, 0.0, 15.0);
    assert_eq!(r.start, 0);
    assert_eq!(r.total, 106.0);
}

#[test]
fn scroll_to_item_alem_do_fim_limita() {
    let h = [40.0; 3];
    assert_eq!(item_offset(&h, 0.0, 99), 120.0);
}

#[test]
fn render_chama_so_os_itens_da_faixa() {
    let mut anim = Animator::default();
    let mut chamados = Vec::new();
    with_cx::<V, _>(&mut anim, |cx| {
        cx.scroll = vec![(ElementId::from("lista"), 2000.0, 39_600.0)];
        cx.scroll_view = vec![(ElementId::from("lista"), 400.0)];
        let heights: Arc<[f32]> = vec![40.0; 1000].into();
        let _ = VirtualList::new("lista", heights).render(cx, |i, _| {
            chamados.push(i);
            div().h(40.0)
        });
    });
    assert_eq!(chamados.first(), Some(&40));
    assert_eq!(chamados.len(), 30);
}

#[test]
fn on_visible_dispara_uma_vez() {
    let root = div().flex_col().child(div().h(50.0).child(on_visible(0.0, |v: &mut V, _| v.vistos.push("a"))));
    let mut t = build::<V>(root);
    let mut v = V::default();
    let mut cx = EventCx::default();
    for f in t.take_visible() {
        f(&mut v, &mut cx);
    }
    assert!(t.take_visible().is_empty(), "consumido na primeira vez");
    assert_eq!(v.vistos, vec!["a"]);
}

#[test]
fn on_visible_fora_nao_dispara() {
    // A janela de teste tem 200 px de altura; o filho está em y = 500.
    let root = div().flex_col().child(div().h(500.0).flex_shrink_0()).child(div().h(20.0).flex_shrink_0().child(on_visible(100.0, |v: &mut V, _| v.vistos.push("b"))));
    let mut t = build::<V>(root);
    assert!(t.take_visible().is_empty());
}

#[test]
fn on_visible_margem_alcanca() {
    let root = div().flex_col().child(div().h(250.0).flex_shrink_0()).child(div().h(20.0).flex_shrink_0().child(on_visible(100.0, |v: &mut V, _| v.vistos.push("c"))));
    let mut t = build::<V>(root);
    assert_eq!(t.take_visible().len(), 1);
}

#[test]
fn ids_sob_o_mouse_com_on_hover() {
    let root = div().flex_col().child(div().id("card").h(50.0).on_hover(|_: &mut V, _, _| {}).child(div().h(20.0)));
    let mut t = build::<V>(root);
    let mut v = V::default();
    t.pointer_move(crate::point(5.0, 5.0), crate::Modifiers::default(), &mut v, &mut EventCx::default());
    assert_eq!(t.hovered_ids(), vec![ElementId::from("card")]);
    t.pointer_move(crate::point(5.0, 150.0), crate::Modifiers::default(), &mut v, &mut EventCx::default());
    assert!(t.hovered_ids().is_empty());
}
