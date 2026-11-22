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

#[test]
fn proporcoes_ficam_na_faixa() {
    let mut r = long(&["Cap 1.cbz"]);
    open(&mut r, "Cap 1.cbz", 40);
    let items = r.strip_items();
    for i in &items {
        if let StripKind::Page { unit, page, .. } = &i.kind {
            r.ratios.insert(super::state::page_key(unit, *page), 0.2);
        }
    }
    r.set_strip_window(&items, 20..22);
    r.request_pages();
    assert_eq!(r.ratios.len(), 40, "altura das páginas não pode mudar ao rolar");
}

#[test]
fn carregando_nao_vira_posicao() {
    let mut r = long(&["Cap 1.cbz", "Cap 2.cbz"]);
    open(&mut r, "Cap 1.cbz", 2);
    r.extend_strip();
    let items = r.strip_items();
    let ix = items.iter().position(|i| matches!(i.kind, StripKind::Loading)).unwrap();
    assert!(!r.on_strip_top(&items, ix));
    assert_eq!(r.chapter(), 0);
}

#[test]
fn capitulo_vazio_na_faixa_vira_erro() {
    let mut r = long(&["Cap 1.cbz", "Cap 2.cbz"]);
    open(&mut r, "Cap 1.cbz", 2);
    r.extend_strip();
    open(&mut r, "Cap 2.cbz", 0);
    assert!(kinds(&r).contains(&"erro1".to_string()), "{:?}", kinds(&r));
}

#[test]
fn tentar_de_novo_no_capitulo_certo() {
    let mut r = long(&["Cap 1.cbz", "Cap 2.cbz"]);
    open(&mut r, "Cap 1.cbz", 2);
    r.extend_strip();
    r.on_opened("Mangás/X/Cap 2.cbz", Err("x".into()));
    assert!(r.strip_opens().is_empty());
    r.retry_chapter(1);
    assert_eq!(r.strip_opens().len(), 1, "pede de novo o capítulo que falhou");
}

#[test]
fn trocar_modo_limpa_janela_da_faixa() {
    let mut r = long(&["Cap 1.cbz"]);
    open(&mut r, "Cap 1.cbz", 20);
    let items = r.strip_items();
    r.set_strip_window(&items, 0..2);
    r.set_mode(ReadMode::Single);
    assert!(r.strip_keep.is_empty() && r.strip_wants.is_empty());
}
