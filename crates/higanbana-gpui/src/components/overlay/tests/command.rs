//! Command: setas atravessam grupos, Enter confirma, digitar avisa, loading e vazio.
use super::{center, click, key, Harness};
use crate::components::form::test_util::paint;
use crate::components::{Command, CommandGroup, CommandItem, CommandState, Component};
use crate::prelude::*;
use crate::Tree;

#[derive(Default)]
struct V {
    cmd: CommandState,
    groups: Vec<Vec<&'static str>>,
    confirmed: Vec<(usize, usize)>,
    queries: Vec<String>,
}

fn frame(h: &mut Harness, t: &mut Tree<V>, v: &V) {
    h.frame(t, |cx| {
        let mut c = Command::new("busca", &v.cmd, |v: &mut V| &mut v.cmd)
            .placeholder("Buscar…")
            .max_h(300.0)
            .empty(text("Nada encontrado"))
            .on_query(|v: &mut V, q, _| v.queries.push(q.to_owned()))
            .on_confirm(|v: &mut V, ix, _| v.confirmed.push(ix));
        for (g, items) in v.groups.iter().enumerate() {
            let mut grp = CommandGroup::new(format!("Grupo {g}"));
            for (i, label) in items.iter().enumerate() {
                grp = grp.item(CommandItem::new(("res", g * 100 + i)).child(text(*label)));
            }
            c = c.group(grp);
        }
        div().size_full().child(c.render(cx))
    });
}

fn setup(groups: Vec<Vec<&'static str>>) -> (Harness, Tree<V>, V) {
    let (mut h, mut t) = (Harness::new(), Tree::new());
    let v = V { groups, ..V::default() };
    frame(&mut h, &mut t, &v);
    t.focus(Command::<V>::input_id("busca"));
    (h, t, v)
}

fn press(t: &mut Tree<V>, v: &mut V, k: Key) {
    t.key_down(&key(k), v, &mut EventCx::default());
}

#[test]
fn setas_atravessam_grupos() {
    let (mut h, mut t, mut v) = setup(vec![vec!["a", "b"], vec!["c"]]);
    press(&mut t, &mut v, Key::Down);
    press(&mut t, &mut v, Key::Down);
    assert_eq!(v.cmd.selected(), Some((1, 0)));
    frame(&mut h, &mut t, &v);
    press(&mut t, &mut v, Key::Up);
    assert_eq!(v.cmd.selected(), Some((0, 1)));
}

#[test]
fn setas_param_nas_pontas() {
    let (_h, mut t, mut v) = setup(vec![vec!["a", "b"]]);
    press(&mut t, &mut v, Key::Up);
    assert_eq!(v.cmd.selected(), Some((0, 0)), "primeiro já era o selecionado");
    for _ in 0..5 {
        press(&mut t, &mut v, Key::Down);
    }
    assert_eq!(v.cmd.selected(), Some((0, 1)));
}

#[test]
fn setas_com_lista_vazia_nao_quebram() {
    let (_h, mut t, mut v) = setup(vec![vec![], vec![]]);
    press(&mut t, &mut v, Key::Down);
    press(&mut t, &mut v, Key::Up);
    press(&mut t, &mut v, Key::Enter);
    assert_eq!(v.cmd.selected(), None);
    assert!(v.confirmed.is_empty());
}

#[test]
fn enter_confirma_selecionado() {
    let (_h, mut t, mut v) = setup(vec![vec!["a"], vec!["b", "c"]]);
    press(&mut t, &mut v, Key::Down);
    press(&mut t, &mut v, Key::Enter);
    assert_eq!(v.confirmed, vec![(1, 0)]);
}

#[test]
fn clique_confirma_item() {
    let (_h, mut t, mut v) = setup(vec![vec!["a", "b"]]);
    let p = center(&t, ("res", 1usize));
    click(&mut t, &mut v, p);
    assert_eq!(v.confirmed, vec![(0, 1)]);
}

#[test]
fn digitar_chama_on_query() {
    let (_h, mut t, mut v) = setup(vec![]);
    t.char_input('n', &mut v, &mut EventCx::default());
    t.char_input('a', &mut v, &mut EventCx::default());
    assert_eq!(v.queries, vec!["n".to_string(), "na".to_string()]);
    assert_eq!(v.cmd.query(), "na");
}

#[test]
fn loading_mostra_spinner_e_esconde_vazio() {
    let (mut h, mut t, mut v) = setup(vec![]);
    assert!(paint(&t).texts.iter().any(|s| s == "Nada encontrado"));
    v.cmd.set_loading(true);
    frame(&mut h, &mut t, &v);
    assert!(!paint(&t).texts.iter().any(|s| s == "Nada encontrado"), "buscando não é vazio");
    assert!(t.find(Command::<V>::spinner_id("busca")).is_some());
}

#[test]
fn resultados_novos_voltam_ao_primeiro() {
    let (mut h, mut t, mut v) = setup(vec![vec!["a", "b", "c"]]);
    press(&mut t, &mut v, Key::Down);
    press(&mut t, &mut v, Key::Down);
    v.groups = vec![vec!["x"]];
    frame(&mut h, &mut t, &v);
    press(&mut t, &mut v, Key::Enter);
    assert_eq!(v.confirmed, vec![(0, 0)], "seleção fora do intervalo cai no primeiro");
}
