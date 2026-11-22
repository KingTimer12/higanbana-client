//! Cache de páginas do leitor: janela em volta da página atual, respostas atrasadas.
use higanbana_gpui::ImageData;

use super::load::deliver_page;
use super::tests_nav::{open, pages, reader};
use crate::state::Img;

fn pixel() -> Option<(higanbana_gpui::ImageSource, f32)> {
    Some((ImageData::from_rgba8(1, 1, vec![0, 0, 0, 255]), 0.7))
}

#[test]
fn pagina_atrasada_descartada() {
    let mut slot = None;
    deliver_page(&mut slot, "page:x:0", pixel());
    assert!(slot.is_none(), "leitor fechado: resposta some sem pânico");
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    open(&mut r, "Cap 1.cbz", 10);
    let mut slot = Some(r);
    deliver_page(&mut slot, "page:nunca-pedida:3", pixel());
    assert_eq!(slot.as_ref().unwrap().pages.len(), 0, "página não pedida (ou já solta) não entra");
}

#[test]
fn janela_solta_paginas_antigas() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    open(&mut r, "Cap 1.cbz", 60);
    let mut slot = Some(r);
    for _ in 0..50 {
        let asked = slot.as_mut().unwrap().request_pages();
        for w in asked {
            deliver_page(&mut slot, &w.key, pixel());
        }
        slot.as_mut().unwrap().next();
    }
    let r = slot.as_mut().unwrap();
    r.request_pages();
    assert!(r.pages.len() <= 6, "cache com {} páginas", r.pages.len());
    assert!(r.ratios.len() <= 6, "proporções também soltas");
}

#[test]
fn falha_vira_tentar_de_novo() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    open(&mut r, "Cap 1.cbz", 2);
    let asked = r.request_pages();
    let key = asked[0].key.clone();
    let mut slot = Some(r);
    deliver_page(&mut slot, &key, None);
    let r = slot.as_mut().unwrap();
    assert!(matches!(r.pages.get(&key), Some(Img::Failed)));
    assert!(r.request_pages().is_empty(), "falha não repete sozinha");
    r.retry_page(&key);
    assert_eq!(r.request_pages().len(), 1);
}

#[test]
fn capitulo_sem_paginas_nao_trava() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    r.on_opened("Mangás/X/Cap 1.cbz", Ok(pages(0)));
    assert!(r.empty_chapter());
    let w = r.wants();
    assert!(w.open.is_empty() && w.pages.is_empty());
}
