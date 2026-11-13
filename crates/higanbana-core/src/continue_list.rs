//! Itens do bloco "Continuar" da entrada: o arquivo mais recente de cada obra,
//! a partir do progresso salvo do player e do leitor. Tudo local, sem rede.

use std::collections::HashMap;

use crate::{
    library::{Place, classify, is_dated, parent_path},
    player::store::Progress,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ContinueItem {
    /// Pasta da obra (em franquia, a subpasta datada da obra).
    pub work_path: String,
    pub file_path: String,
    pub updated_at: i64,
    /// Visto até o fim / capítulo lido.
    pub done: bool,
    /// 0..=1.
    pub fraction: f32,
    /// Segundos que faltam (só vídeo não visto).
    pub remaining_secs: Option<u64>,
}

/// Pasta da obra de um arquivo.
pub fn work_key(file_path: &str) -> String {
    match classify(file_path) {
        Place::Title { title_path, rest, .. } => match rest.first() {
            // `rest` inclui o próprio arquivo: só é subpasta de obra se houver algo depois.
            Some(work) if rest.len() > 1 && is_dated(work) => format!("{title_path}/{work}"),
            _ => title_path,
        },
        _ => parent_path(file_path),
    }
}

/// Mantém o item mais recente de cada obra; ordena do mais recente e corta.
fn latest_per_work(items: impl Iterator<Item = ContinueItem>, limit: usize) -> Vec<ContinueItem> {
    let mut by_work: HashMap<String, ContinueItem> = HashMap::new();
    for item in items {
        match by_work.get(&item.work_path) {
            Some(old) if old.updated_at >= item.updated_at => {}
            _ => {
                by_work.insert(item.work_path.clone(), item);
            }
        }
    }
    let mut v: Vec<ContinueItem> = by_work.into_values().collect();
    v.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.work_path.cmp(&b.work_path)));
    v.truncate(limit);
    v
}

/// Episódios: o mais recente de cada obra, mais recentes primeiro.
pub fn watching(progress: &HashMap<String, Progress>, limit: usize) -> Vec<ContinueItem> {
    let items = progress.iter().map(|(path, p)| ContinueItem {
        work_path: work_key(path),
        file_path: path.clone(),
        updated_at: p.updated_at,
        done: p.watched,
        fraction: if p.duration > 0. { (p.position / p.duration).clamp(0., 1.) as f32 } else { 0. },
        remaining_secs: (!p.watched && p.duration > 0.).then(|| (p.duration - p.position).max(0.).round() as u64),
    });
    latest_per_work(items, limit)
}

/// Capítulos: o mais recente de cada obra, mais recentes primeiro.
#[cfg(feature = "reader")]
pub fn reading(progress: &HashMap<String, crate::reader::store::ReadProgress>, limit: usize) -> Vec<ContinueItem> {
    let items = progress.iter().map(|(path, p)| ContinueItem {
        work_path: work_key(path),
        file_path: path.clone(),
        updated_at: p.updated_at,
        done: p.read,
        fraction: if p.total > 0 { (p.page as f32 / p.total as f32).clamp(0., 1.) } else { 0. },
        remaining_secs: None,
    });
    latest_per_work(items, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(position: f64, duration: f64, watched: bool, at: i64) -> Progress {
        Progress { position, duration, watched, updated_at: at }
    }

    #[test]
    fn latest_episode_per_work() {
        let m = HashMap::from([
            ("Animes/Letra F/Frieren/1080p/ep1.mkv".to_string(), p(1., 2., true, 10)),
            ("Animes/Letra F/Frieren/1080p/ep2.mkv".to_string(), p(1., 2., false, 20)),
        ]);
        let v = watching(&m, 12);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].file_path, "Animes/Letra F/Frieren/1080p/ep2.mkv");
        assert_eq!(v[0].work_path, "Animes/Letra F/Frieren");
    }

    #[test]
    fn franchise_works_are_separate() {
        let m = HashMap::from([
            ("Animes/Letra P/Precure/[2004] Futari wa [TV]/a/1.mkv".to_string(), p(1., 2., false, 10)),
            ("Animes/Letra P/Precure/[2010] Heartcatch [TV]/b/1.mkv".to_string(), p(1., 2., false, 20)),
        ]);
        let v = watching(&m, 12);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].work_path, "Animes/Letra P/Precure/[2010] Heartcatch [TV]");
        assert_eq!(v[1].work_path, "Animes/Letra P/Precure/[2004] Futari wa [TV]");
    }

    #[test]
    fn ordered_by_recent_and_limited() {
        let m = HashMap::from([
            ("Animes/Letra A/A/1.mkv".to_string(), p(1., 2., false, 10)),
            ("Animes/Letra B/B/1.mkv".to_string(), p(1., 2., false, 30)),
            ("Animes/Letra C/C/1.mkv".to_string(), p(1., 2., false, 20)),
        ]);
        let v: Vec<String> = watching(&m, 2).into_iter().map(|i| i.work_path).collect();
        assert_eq!(v, vec!["Animes/Letra B/B", "Animes/Letra C/C"]);
    }

    #[cfg(feature = "reader")]
    #[test]
    fn reading_uses_page_fraction() {
        let r = crate::reader::store::ReadProgress { page: 5, total: 20, read: false, updated_at: 3, unit: None, unit_name: None };
        let m = HashMap::from([("Mangás/Séries Mangás/Letra G/Gantz G (2015)/v01.cbz".to_string(), r)]);
        let v = reading(&m, 12);
        assert_eq!(v[0].work_path, "Mangás/Séries Mangás/Letra G/Gantz G (2015)");
        assert_eq!(v[0].fraction, 0.25);
        assert!(!v[0].done && v[0].remaining_secs.is_none());
    }

    #[test]
    fn outside_title_uses_parent() {
        assert_eq!(work_key("Doramas/X/ep1.mkv"), "Doramas/X");
    }

    #[test]
    fn remaining_only_when_unwatched() {
        let m = HashMap::from([("Animes/Letra A/A/1.mkv".to_string(), p(600., 1440., false, 1))]);
        assert_eq!(watching(&m, 12)[0].remaining_secs, Some(840));
        let m = HashMap::from([("Animes/Letra A/A/1.mkv".to_string(), p(1400., 1440., true, 1))]);
        assert_eq!(watching(&m, 12)[0].remaining_secs, None);
    }
}
