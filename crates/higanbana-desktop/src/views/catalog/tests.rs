use std::sync::Arc;

use higanbana_core::anime_index::IndexEntry;

use super::rows::{flat_rows, header_label, letter_at, letter_row, letter_rows, rail_label, search_sections, Body, Metrics, Row};
use super::{reopen_at, Reopen};
use crate::state::CatalogPlace;

fn entry(letter: &str, name: &str) -> IndexEntry {
    IndexEntry { letter: letter.into(), name: name.into(), path: format!("Animes/{letter}/{name}") }
}

#[test]
fn labels() {
    assert_eq!((rail_label("0-9"), rail_label("Letra F")), ("#", "F"));
    assert_eq!((header_label("0-9"), header_label("Letra Z")), ("0–9", "Z"));
}

#[test]
fn search_hides_empty_letters() {
    let f = vec![entry("Letra F", "Fresh Precure!"), entry("Letra F", "Fullmetal Alchemist")];
    let h = vec![entry("Letra H", "Heartcatch PreCure!")];
    let n = vec![entry("Letra N", "Naruto")];
    let found = search_sections([(6, f.as_slice()), (8, h.as_slice()), (14, n.as_slice())], "précure");
    let names: Vec<(usize, Vec<&str>)> = found.iter().map(|(ix, e)| (*ix, e.iter().map(|e| e.name.as_str()).collect())).collect();
    assert_eq!(names, vec![(6, vec!["Fresh Precure!"]), (8, vec!["Heartcatch PreCure!"])]);
}

#[test]
fn voltar_restaura_rolagem_e_filtros() {
    let place = CatalogPlace { upto: 9, top: Some(7), offset: 1200. };
    assert_eq!(reopen_at(Some(3), &place, true), Reopen::Letter(3));
    assert_eq!(reopen_at(None, &place, true), Reopen::Letter(7));
    assert_eq!(reopen_at(None, &CatalogPlace { top: None, ..place }, true), Reopen::Top);
    assert_eq!(reopen_at(None, &place, false), Reopen::Offset(1200.));
    assert_eq!(reopen_at(None, &CatalogPlace::default(), false), Reopen::Top);
    // Filtros e ordenação vêm do que foi guardado.
    let saved = crate::state::CatalogChoice { sort: higanbana_core::catalog_filter::Sort::Year, ..Default::default() };
    let ui = super::CatalogUi::new(&saved, higanbana_core::anilist::MediaType::Anime, None);
    assert!(ui.flat_mode());
}

#[test]
fn letras_viram_linhas_de_grade() {
    let a: Arc<Vec<IndexEntry>> = Arc::new((0..7).map(|i| entry("0-9", &format!("{i}"))).collect());
    let rows = letter_rows(vec![(0, Body::Entries(a)), (1, Body::Loading)], 3, 1);
    // Cabeçalho, título 0-9, 3 linhas (3+3+1), título A, esqueleto, rodapé sem próxima (A carregando).
    assert_eq!(rows.len(), 8);
    assert!(matches!(rows[2], Row::Cards { start: 0, end: 3, .. }));
    assert!(matches!(rows[4], Row::Cards { start: 6, end: 7, .. }));
    assert!(matches!(rows[7], Row::Footer { next: None }));
    assert_eq!(letter_row(&rows, 1), Some(5));
}

#[test]
fn letra_no_topo_pela_rolagem() {
    let a: Arc<Vec<IndexEntry>> = Arc::new(vec![entry("0-9", "1")]);
    let b: Arc<Vec<IndexEntry>> = Arc::new(vec![entry("Letra A", "Aria")]);
    let rows = letter_rows(vec![(0, Body::Entries(a)), (1, Body::Entries(b))], 4, 1);
    let m = Metrics { cols: 4, card_w: 100.0, header_h: 200.0 };
    let h: Vec<f32> = rows.iter().map(|r| m.height(r)).collect();
    assert_eq!(letter_at(&rows, &h, 0.0), None, "cabeçalho no topo");
    assert_eq!(letter_at(&rows, &h, 210.0), Some(0));
    let a_top: f32 = h[..3].iter().sum();
    assert_eq!(letter_at(&rows, &h, a_top + 5.0), Some(1));
    assert!(matches!(rows.last(), Some(Row::Footer { next: Some(2) })), "última carregada pede a próxima");
}

#[test]
fn busca_filtra_sem_rede() {
    let works: Arc<[IndexEntry]> = vec![entry("Letra C", "Clannad")].into();
    let rows = flat_rows(works, 5, false);
    assert_eq!(rows.len(), 3);
}
