//! `line_clamp`: altura limitada a N linhas e texto cortado com reticências.
use super::tests::build;
use crate::prelude::*;

struct V;

fn long() -> Element<V> {
    // 50 caracteres = 5 linhas de 10 em 70 px (medidor falso: 7 px/char, 16 px/linha).
    div().w(70.0).flex_col().child(text("a".repeat(50)))
}

#[test]
fn clamp_limita_altura() {
    let t = build::<V>(long().line_clamp(2), 300.0, 300.0);
    assert_eq!(t.rect(1).size.height, 32.0);
    let livre = build::<V>(long(), 300.0, 300.0);
    assert_eq!(livre.rect(1).size.height, 80.0);
}

#[test]
fn clamp_poe_reticencias() {
    let t = build::<V>(long().line_clamp(2), 300.0, 300.0);
    let s = t.text_shown(1);
    assert!(s.ends_with('…'), "{s}");
    assert!(s.chars().count() <= 20);
}

#[test]
fn clamp_texto_curto_fica_igual() {
    let t = build::<V>(div().w(70.0).flex_col().line_clamp(2).child(text("abc")), 300.0, 300.0);
    assert_eq!(t.text_shown(1), "abc");
    assert_eq!(t.rect(1).size.height, 16.0);
}

#[test]
fn clamp_1_corta_numa_linha() {
    let t = build::<V>(long().line_clamp(1), 300.0, 300.0);
    assert_eq!(t.rect(1).size.height, 16.0);
    assert!(t.text_shown(1).ends_with('…'));
}
