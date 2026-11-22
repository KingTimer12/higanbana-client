//! Navegação do leitor sem janela nem rede.
use std::sync::Arc;

use higanbana_core::nuvem::FileEntry;
use higanbana_core::reader::api::{OpenResult, PageInfo};
use higanbana_core::reader::logic::{Direction, ReadMode};
use higanbana_core::reader::store::{ReadProgress, ReaderData};

use super::nav::Step;
use super::state::ReaderUi;
use super::ReadTarget;

fn cbz(name: &str) -> FileEntry {
    FileEntry { name: name.into(), is_directory: false, size: 100, modified: String::new(), extension: Some(".cbz".into()) }
}

fn pages(n: usize) -> OpenResult {
    OpenResult { pages: (0..n).map(|i| PageInfo { index: i, name: format!("{i:03}.jpg") }).collect(), archives: vec![] }
}

fn reader(files: &[&str], start: &str) -> ReaderUi {
    let files: Arc<[FileEntry]> = files.iter().map(|f| cbz(f)).collect();
    let file = files.iter().find(|f| f.name == start).unwrap().clone();
    let t = ReadTarget { dir: "Mangás/X".into(), file, files, title_path: "Mangás/X".into(), title: "X".into() };
    ReaderUi::new(t, &ReaderData::default())
}

fn open(r: &mut ReaderUi, name: &str, n: usize) {
    r.on_opened(&format!("Mangás/X/{name}"), Ok(pages(n)));
}

#[test]
fn avanca_e_volta_pagina() {
    let mut r = reader(&["Cap 1.cbz", "Cap 2.cbz"], "Cap 1.cbz");
    open(&mut r, "Cap 1.cbz", 3);
    assert_eq!(r.page(), 0);
    assert_eq!(r.next(), Step::Moved);
    assert_eq!(r.page(), 1);
    r.prev();
    assert_eq!(r.page(), 0);
    assert_eq!(r.prev(), Step::None, "primeira página do primeiro capítulo");
}

#[test]
fn fim_do_capitulo_mostra_cartao() {
    let mut r = reader(&["Cap 1.cbz", "Cap 2.cbz"], "Cap 1.cbz");
    r.settings.seam = false;
    open(&mut r, "Cap 1.cbz", 2);
    r.next();
    r.next();
    assert!(r.end);
    r.next();
    assert_eq!(r.chapter(), 1, "no cartão, avançar vai ao próximo");
    assert!(!r.end);
}

#[test]
fn emenda_vira_direto_para_o_proximo() {
    let mut r = reader(&["Cap 1.cbz", "Cap 2.cbz"], "Cap 1.cbz");
    open(&mut r, "Cap 1.cbz", 1);
    r.next();
    assert_eq!((r.chapter(), r.end), (1, false));
}

#[test]
fn pagina_dupla_salva_a_mais_adiantada() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    r.set_mode(ReadMode::Double);
    open(&mut r, "Cap 1.cbz", 5);
    r.next(); // capa sozinha → par [1, 2]
    assert_eq!(r.spread(), vec![1, 2]);
    let s = r.progress_to_save().unwrap();
    assert_eq!((s.page, s.total), (2, 5));
}

#[test]
fn pagina_none_vai_para_ultima_ao_abrir() {
    let mut r = reader(&["Cap 1.cbz", "Cap 2.cbz"], "Cap 2.cbz");
    open(&mut r, "Cap 2.cbz", 2);
    r.prev();
    assert_eq!(r.chapter(), 0);
    open(&mut r, "Cap 1.cbz", 4);
    assert_eq!(r.page(), 3, "voltar de capítulo abre na última página");
}

#[test]
fn divisao_mantem_capitulo() {
    let mut r = reader(&["Vol 1.cbz", "Vol 2.cbz"], "Vol 1.cbz");
    // Vol 1 tem duas pastas: vira dois capítulos.
    let mut res = pages(0);
    res.pages = vec![PageInfo { index: 0, name: "c1/01.jpg".into() }, PageInfo { index: 1, name: "c1/02.jpg".into() }, PageInfo { index: 2, name: "c2/01.jpg".into() }];
    r.progress.insert("Mangás/X/Vol 1.cbz".into(), ReadProgress { page: 0, total: 2, read: false, updated_at: 0, unit: Some("Mangás/X/Vol 1.cbz#c2".into()), unit_name: None });
    r.on_opened("Mangás/X/Vol 1.cbz", Ok(res));
    assert_eq!(r.chapters.len(), 3);
    assert_eq!(r.cur_key, "Mangás/X/Vol 1.cbz#c2", "retoma no capítulo interno salvo");
    assert_eq!(r.total(), 1);
    let s = r.progress_to_save().unwrap();
    assert_eq!(s.key, "Mangás/X/Vol 1.cbz#c2");
    let sum = s.summary.unwrap();
    assert_eq!((sum.path.as_str(), sum.index, sum.count), ("Mangás/X/Vol 1.cbz", 1, 2));
}

#[test]
fn wants_pre_carrega_tres() {
    let mut r = reader(&["Cap 1.cbz", "Cap 2.cbz"], "Cap 1.cbz");
    assert_eq!(r.wants().open.len(), 1, "capítulo atual ainda fechado");
    open(&mut r, "Cap 1.cbz", 10);
    let w = r.wants();
    assert_eq!(w.pages.iter().map(|p| p.page).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert!(w.open.is_empty());
}

#[test]
fn wants_abre_proximo_perto_do_fim() {
    let mut r = reader(&["Cap 1.cbz", "Cap 2.cbz"], "Cap 1.cbz");
    open(&mut r, "Cap 1.cbz", 3);
    let w = r.wants();
    assert_eq!(w.open.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), vec!["Cap 2.cbz"]);
}

#[test]
fn tecla_d_inverte_direcao() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    assert_eq!(r.prefs.direction, Direction::Rtl, "títulos novos começam da direita");
    r.toggle_direction();
    assert_eq!(r.prefs.direction, Direction::Ltr);
}

#[test]
fn faixa_devolve_rolagem() {
    let mut r = reader(&["Cap 1.cbz"], "Cap 1.cbz");
    r.set_mode(ReadMode::Long);
    assert_eq!(r.next(), Step::Scroll(1.0));
    assert_eq!(r.pending_scroll, Some(("Mangás/X/Cap 1.cbz".into(), 0)));
}
