//! Painéis do leitor (índice), fio de progresso e realce ao mexer o mouse.
use higanbana_core::reader::logic::{base_unit, Unit};
use higanbana_gpui::EventCx;

use super::tests_nav::{cbz, reader};
use super::view::panels::{count_word, index_items};
use super::view::thread::shows_stamens;

fn units(names: &[&str]) -> Vec<Unit> {
    names.iter().map(|n| base_unit(&format!("X/{n}"), &cbz(n))).collect()
}

#[test]
fn indice_filtra_por_nome_e_rotulo() {
    let u = units(&["Capitulo 1.cbz", "Capitulo 12.cbz", "Extra.cbz"]);
    assert_eq!(index_items(&u, ""), vec![0, 1, 2]);
    assert_eq!(index_items(&u, " EXTRA "), vec![2], "nome, sem caixa nem espaços");
    assert_eq!(index_items(&u, "cap. 12"), vec![1], "rótulo normalizado");
}

#[test]
fn palavra_do_contador() {
    assert_eq!(count_word(&units(&["Vol. 1.cbz", "Vol. 2.cbz"])), "volumes");
    assert_eq!(count_word(&units(&["Capitulo 1.cbz"])), "capítulo");
    assert_eq!(count_word(&units(&["Vol. 1.cbz", "Capitulo 2.cbz"])), "arquivos");
}

#[test]
fn mexer_repoe_um_timer_so() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    let mut cx = EventCx::default();
    let first = r.stir(&mut cx);
    assert!(first, "primeiro movimento redesenha");
    let id = r.stir_timer;
    assert!(!r.stir(&mut cx), "já realçado: nada a redesenhar");
    assert_ne!(r.stir_timer, id, "timer reposto (o anterior foi cancelado)");
    assert!(r.stirred);
}

#[test]
fn fio_some_acima_de_120() {
    assert!(shows_stamens(120));
    assert!(!shows_stamens(121));
}
