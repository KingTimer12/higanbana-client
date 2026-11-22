use higanbana_core::nuvem::FileEntry;
use higanbana_core::tmdb::EpisodeView;

use super::labels::{air_date, card_text, disambiguators, head_number, kicker, list_columns};

fn file(name: &str) -> FileEntry {
    FileEntry { name: name.to_owned(), is_directory: false, size: 0, modified: String::new(), extension: Some(".mkv".into()) }
}

fn episode(number: u32, title: &str) -> EpisodeView {
    EpisodeView {
        season: 1,
        number,
        title: title.to_owned(),
        overview: String::new(),
        air_date: None,
        still_path: None,
        vote_average: 0.,
        vote_count: 0,
        runtime: None,
    }
}

#[test]
fn card_text_with_episode() {
    let f = file("[Grupo] Sousou no Frieren - 01 [1080p].mkv");
    assert_eq!(card_text(&f, Some(&episode(1, "O fim da aventura")), false), ("E1".into(), "O fim da aventura".into()));
    // Filme: o número do nome do arquivo não vira rótulo.
    let f = file("One Piece - Filme 02 [BD_1080p].mkv");
    assert_eq!(card_text(&f, Some(&episode(1, "Aventura na Ilha Relógio")), true), (String::new(), "Aventura na Ilha Relógio".into()));
    assert_eq!(card_text(&f, None, true), ("E2".into(), "Episódio 2".into()));
}

#[test]
fn card_text_without_episode_uses_number() {
    let f = file("[Grupo] Sousou no Frieren - 05 [1080p].mkv");
    assert_eq!(card_text(&f, None, false), ("E5".into(), "Episódio 5".into()));
}

#[test]
fn card_text_without_number_uses_file_name() {
    let f = file("Abertura sem créditos.mkv");
    assert_eq!(card_text(&f, None, false), (String::new(), "Abertura sem créditos.mkv".into()));
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(l, n)| (l.to_string(), n.to_string())).collect()
}

#[test]
fn repeated_label_shows_quality() {
    let items = pairs(&[("E1", "Show - 01 [1080p].mkv"), ("E1", "Show - 01 [720p].mkv"), ("E2", "Show - 02 [1080p].mkv")]);
    assert_eq!(disambiguators(&items), [Some("1080p".to_owned()), Some("720p".to_owned()), None]);
}

#[test]
fn repeated_label_with_same_quality_shows_file_name() {
    let items = pairs(&[("E1", "Show - 01 [1080p].mkv"), ("E1", "Show - 01v2 [1080p].mkv")]);
    assert_eq!(disambiguators(&items), [Some("Show - 01 [1080p].mkv".to_owned()), Some("Show - 01v2 [1080p].mkv".to_owned())]);
}

#[test]
fn unique_or_empty_labels_need_nothing() {
    let items = pairs(&[("", "Abertura.mkv"), ("", "Encerramento.mkv"), ("E3", "Show - 03.mkv")]);
    assert_eq!(disambiguators(&items), [None, None, None]);
}

#[test]
fn head_number_pads_the_label() {
    assert_eq!(head_number("E1").as_deref(), Some("01"));
    assert_eq!(head_number("E12").as_deref(), Some("12"));
    assert_eq!(head_number("E105").as_deref(), Some("105"));
    assert_eq!(head_number(""), None);
}

#[test]
fn list_has_half_the_grid_columns() {
    assert_eq!(list_columns(4), 2);
    assert_eq!(list_columns(5), 2);
    assert_eq!(list_columns(1), 1);
    assert_eq!(list_columns(0), 1);
}

#[test]
fn kicker_names_episode_and_date() {
    assert_eq!(kicker("E1", Some("29 set 2023")), "EPISÓDIO 1 · 29 SET 2023");
    assert_eq!(kicker("E7", None), "EPISÓDIO 7");
    assert_eq!(kicker("", Some("29 set 2023")), "29 SET 2023");
    assert_eq!(kicker("", None), "");
}

#[test]
fn air_date_like_anilist() {
    assert_eq!(air_date(Some("2023-09-29")).as_deref(), Some("29 set 2023"));
    assert_eq!(air_date(Some("")), None);
    assert_eq!(air_date(None), None);
}
