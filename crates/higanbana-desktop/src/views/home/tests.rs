use std::time::{Duration, Instant};

use higanbana_core::continue_list::ContinueItem;

use super::continue_row::{item_label, shape_of};
use super::HomeUi;

fn item(done: bool, fraction: f32, remaining_secs: Option<u64>) -> ContinueItem {
    ContinueItem { work_path: "Animes/F/Frieren".into(), file_path: "Animes/F/Frieren/ep.mkv".into(), updated_at: 0, done, fraction, remaining_secs }
}

fn at(work: &str, updated_at: i64) -> ContinueItem {
    ContinueItem { work_path: work.into(), file_path: format!("{work}/x"), updated_at, done: false, fraction: 0.5, remaining_secs: None }
}

#[test]
fn rotulos_do_continuar() {
    assert_eq!(item_label("[G] Frieren - 07 [1080p].mkv", &item(false, 0.4, Some(840)), false), "Ep 07 · faltam 14 min");
    assert_eq!(item_label("Show - 03.mkv", &item(false, 0.99, Some(20)), false), "Ep 03 · falta menos de 1 min");
    assert_eq!(item_label("Show - 03.mkv", &item(true, 1., None), false), "Ep 03 · visto");
    assert_eq!(item_label("Filme.mkv", &item(false, 0.5, Some(3600)), false), "Filme · faltam 60 min");
    assert_eq!(item_label("Gantz G v01.cbz", &item(false, 0.25, None), true), "Gantz G v01 · 25% lido");
    assert_eq!(item_label("Gantz G v01.cbz", &item(true, 1., None), true), "Gantz G v01 · lido");
}

#[test]
fn continue_vazio_some() {
    assert!(shape_of(Vec::new(), Vec::new()).featured.is_none());
}

#[test]
fn destaque_e_o_mais_recente() {
    let s = shape_of(vec![at("a", 5), at("b", 3)], vec![at("c", 9)]);
    let (f, reading) = s.featured.unwrap();
    assert_eq!((f.work_path.as_str(), reading), ("c", true));
    assert_eq!(s.watching.len(), 2);
    assert!(s.reading.is_empty());
}

#[test]
fn hero_avanca_sozinho_e_para_com_o_mouse() {
    let t = Instant::now();
    let mut ui = HomeUi::default();
    ui.hero = ui.hero.clone().started(t);
    ui.hero.set_len(3, t);
    assert_eq!(ui.hero.current(t + Duration::from_secs(8)), 1);
    ui.hero.set_paused(true, t + Duration::from_secs(9));
    assert_eq!(ui.hero.current(t + Duration::from_secs(60)), 1);
    assert_eq!(ui.hero.next_change(t + Duration::from_secs(60)), None, "pausado não agenda frame");
}
