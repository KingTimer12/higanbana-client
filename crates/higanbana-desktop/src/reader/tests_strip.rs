//! Faixa (long/wide strip): itens, emenda, posição pelo topo e janela de páginas.
use higanbana_core::reader::logic::{Direction, ReadMode};

use super::strip::StripKind;
use super::tests_nav::{open, reader};

fn kinds(r: &super::ReaderUi) -> Vec<String> {
    r.strip_items()
        .iter()
        .map(|i| match &i.kind {
            StripKind::Page { page, .. } => format!("p{page}"),
            StripKind::Loading => "carregando".into(),
            StripKind::End(c) => format!("fim{c}"),
            StripKind::Seam(c) => format!("emenda{c}"),
            StripKind::Failed(c, _) => format!("erro{c}"),
        })
        .collect()
}

fn long(files: &[&str]) -> super::ReaderUi {
    let mut r = reader(files, files[0]);
    r.set_mode(ReadMode::Long);
    r
}

#[test]
fn itens_da_faixa_com_emenda() {
    let mut r = long(&["Cap 1.cbz", "Cap 2.cbz"]);
    open(&mut r, "Cap 1.cbz", 2);
    assert_eq!(kinds(&r), ["p0", "p1"], "sem o próximo ainda na faixa: nem fim nem emenda");
    assert!(r.extend_strip(), "perto do fim, emenda o próximo");
    assert_eq!(kinds(&r), ["p0", "p1", "emenda0", "carregando", "fim1"]);
    open(&mut r, "Cap 2.cbz", 1);
    assert_eq!(kinds(&r), ["p0", "p1", "emenda0", "p0", "fim1"]);
}

#[test]
fn wide_rtl_inverte() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    r.set_mode(ReadMode::Wide);
    r.prefs.direction = Direction::Rtl;
    open(&mut r, "Cap 1.cbz", 2);
    assert_eq!(kinds(&r), ["fim0", "p1", "p0"], "primeira página à direita");
}

#[test]
fn topo_atualiza_pagina_e_salva() {
    let mut r = long(&["Cap 1.cbz", "Cap 2.cbz"]);
    open(&mut r, "Cap 1.cbz", 2);
    r.extend_strip();
    open(&mut r, "Cap 2.cbz", 3);
    let items = r.strip_items();
    let ix = items.iter().position(|i| i.key == ("Mangás/X/Cap 2.cbz".to_string(), 1)).unwrap();
    assert!(r.on_strip_top(&items, ix));
    assert_eq!((r.chapter(), r.page()), (1, 1));
    assert_eq!(r.progress_to_save().unwrap().key, "Mangás/X/Cap 2.cbz");
    assert!(!r.on_strip_top(&items, ix), "mesmo item: nada muda");
}

#[test]
fn emenda_so_com_opcao_ligada() {
    let mut r = long(&["Cap 1.cbz", "Cap 2.cbz"]);
    r.settings.seam = false;
    open(&mut r, "Cap 1.cbz", 2);
    assert!(!r.extend_strip());
    assert_eq!(kinds(&r), ["p0", "p1", "fim0"]);
}

#[test]
fn capitulo_falhou_na_faixa_mostra_erro() {
    let mut r = long(&["Cap 1.cbz"]);
    r.on_opened("Mangás/X/Cap 1.cbz", Err("sem rede".into()));
    assert_eq!(kinds(&r), ["erro0"]);
}

#[test]
fn janela_da_faixa_pede_so_perto_do_visivel() {
    let mut r = long(&["Cap 1.cbz"]);
    open(&mut r, "Cap 1.cbz", 40);
    let items = r.strip_items();
    r.set_strip_window(&items, 20..22);
    let asked = r.request_pages();
    let pages: Vec<usize> = asked.iter().map(|w| w.page).collect();
    assert_eq!(pages.first(), Some(&14));
    assert_eq!(pages.last(), Some(&27));
    r.set_strip_window(&items, 0..2);
    r.request_pages();
    assert!(r.pages.len() <= 14, "solta o que saiu da janela");
}
