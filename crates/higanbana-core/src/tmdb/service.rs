//! Episódios de uma obra pelo TMDB, com cache em disco. Erro de rede nunca vai para o cache.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    TmdbError, TmdbResult,
    client::TmdbClient,
    matching::{Anchor, WorkKind, WorkQuery, map_file, pick_anchor, pick_overview, pick_show, pick_text},
    types::{Episode, TvShow},
};
use crate::{
    anilist::cache::DiskCache,
    player::logic::{episode_number, episode_season},
};

const PT: &str = "pt-BR";
const EN: &str = "en-US";
const WORKS_TTL: i64 = 7 * 24 * 3600;
const DAY_TTL: i64 = 24 * 3600;

/// Episódio já com título e sinopse resolvidos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeView {
    pub season: u32,
    pub number: u32,
    pub title: String,
    pub overview: String,
    pub air_date: Option<String>,
    pub still_path: Option<String>,
    pub vote_average: f32,
    pub vote_count: u32,
    pub runtime: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkEpisodes {
    pub show_id: u64,
    pub show_vote: (f32, u32),
    pub anchor: Anchor,
    pub show: TvShow,
    pub episodes: Vec<EpisodeView>,
}

impl WorkEpisodes {
    /// Episódio do TMDB que corresponde ao arquivo (número, temporada explícita e âncora).
    pub fn link(&self, file_name: &str) -> Option<&EpisodeView> {
        let number = episode_number(file_name)?;
        let (season, n) = map_file(self.anchor, &self.show, episode_season(file_name), number)?;
        self.episodes.iter().find(|e| e.season == season && e.number == n)
    }
}

fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Chave de cache da obra (e do mapa no desktop): títulos normalizados, ano, início e tipo.
pub fn work_key(q: &WorkQuery) -> String {
    let titles: Vec<String> = q.titles.iter().map(|t| normalize(t)).filter(|t| !t.is_empty()).collect();
    let kind = match q.kind {
        WorkKind::Series => "series",
        WorkKind::Special => "special",
    };
    let year = q.year.map(|y| y.to_string()).unwrap_or_default();
    let start = q.start.map(|s| format!("{}-{:02}-{:02}", s.y, s.m, s.d)).unwrap_or_default();
    format!("{}|{year}|{start}|{kind}", titles.join("|"))
}

pub struct Tmdb {
    client: TmdbClient,
    works: DiskCache<Option<(u64, Anchor)>>,
    shows: DiskCache<TvShow>,
    seasons: DiskCache<Vec<Episode>>,
}

impl Tmdb {
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            client: TmdbClient::new(),
            works: DiskCache::open(cache_dir.join("tmdb-works.json"), WORKS_TTL),
            shows: DiskCache::open(cache_dir.join("tmdb-shows.json"), DAY_TTL),
            seasons: DiskCache::open(cache_dir.join("tmdb-seasons.json"), DAY_TTL),
        }
    }

    pub fn configured(&self) -> bool {
        self.client.configured()
    }

    /// `Ok(None)`: o TMDB não tem a obra (ou não dá para casar com segurança).
    pub async fn work_episodes(&self, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>> {
        let key = work_key(q);
        let result = self.resolve(&key, q).await;
        self.works.flush();
        self.shows.flush();
        self.seasons.flush();
        result
    }

    async fn resolve(&self, key: &str, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>> {
        let (show_id, known) = match self.works.get(key) {
            Some(None) => return Ok(None),
            Some(Some((id, anchor))) => (id, Some(anchor)),
            None => match self.find_show(q).await? {
                Some(id) => (id, None),
                None => {
                    self.works.insert(key.to_owned(), None);
                    return Ok(None);
                }
            },
        };
        let show = self.show(show_id).await?;
        let mut pt = Vec::new();
        for season in wanted_seasons(&show, q.kind) {
            pt.push((season, self.season(show_id, season, PT).await?));
        }
        let anchor = match known {
            Some(a) => a,
            None => match pick_anchor(&show, &pt, q) {
                Some(a) => {
                    self.works.insert(key.to_owned(), Some((show_id, a)));
                    a
                }
                None => {
                    self.works.insert(key.to_owned(), None);
                    return Ok(None);
                }
            },
        };
        let mut episodes = Vec::new();
        for (season, list_pt) in &pt {
            let list_en = self.season(show_id, *season, EN).await?;
            episodes.extend(list_pt.iter().map(|e| {
                let en = list_en.iter().find(|x| x.episode_number == e.episode_number);
                EpisodeView {
                    season: *season,
                    number: e.episode_number,
                    title: pick_text(&e.name, en.map_or("", |x| x.name.as_str()), e.episode_number),
                    overview: pick_overview(&e.overview, en.map_or("", |x| x.overview.as_str())),
                    air_date: e.air_date.clone().filter(|d| !d.is_empty()),
                    still_path: e.still_path.clone().filter(|p| !p.is_empty()),
                    vote_average: e.vote_average,
                    vote_count: e.vote_count,
                    runtime: e.runtime,
                }
            }));
        }
        Ok(Some(WorkEpisodes { show_id, show_vote: (show.vote_average, show.vote_count), anchor, show, episodes }))
    }

    async fn find_show(&self, q: &WorkQuery) -> TmdbResult<Option<u64>> {
        for title in q.titles.iter().filter(|t| !t.trim().is_empty()) {
            let hits = self.client.search_tv(title, PT).await?;
            if let Some(id) = pick_show(&hits, q) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    async fn show(&self, id: u64) -> TmdbResult<TvShow> {
        let key = id.to_string();
        if let Some(s) = self.shows.get(&key) {
            return Ok(s);
        }
        let show = self.client.tv(id, PT).await?;
        self.shows.insert(key, show.clone());
        Ok(show)
    }

    async fn season(&self, id: u64, season: u32, lang: &str) -> TmdbResult<Vec<Episode>> {
        let key = format!("{id}:{season}:{lang}");
        if let Some(list) = self.seasons.get(&key) {
            return Ok(list);
        }
        let list = match self.client.season(id, season, lang).await {
            Ok(list) => list,
            Err(TmdbError::NotFound) => Vec::new(),
            Err(e) => return Err(e),
        };
        self.seasons.insert(key, list.clone());
        Ok(list)
    }
}

/// Temporadas a buscar: as regulares (âncora e numeração absoluta) ou só a 0 nos especiais.
fn wanted_seasons(show: &TvShow, kind: WorkKind) -> Vec<u32> {
    let mut seasons: Vec<u32> = show
        .seasons
        .iter()
        .map(|s| s.season_number)
        .filter(|&s| match kind {
            WorkKind::Series => s > 0,
            WorkKind::Special => s == 0,
        })
        .collect();
    seasons.sort_unstable();
    seasons.dedup();
    seasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmdb::types::SeasonInfo;

    fn view(season: u32, number: u32) -> EpisodeView {
        EpisodeView {
            season,
            number,
            title: format!("T{number}"),
            overview: String::new(),
            air_date: None,
            still_path: None,
            vote_average: 0.0,
            vote_count: 0,
            runtime: None,
        }
    }

    #[test]
    fn link_uses_anchor() {
        let show = TvShow {
            id: 1,
            name: "X".into(),
            vote_average: 0.0,
            vote_count: 0,
            seasons: vec![SeasonInfo { season_number: 1, episode_count: 24, air_date: None }],
        };
        let w = WorkEpisodes {
            show_id: 1,
            show_vote: (0.0, 0),
            anchor: Anchor { season: 1, offset: 12 },
            show,
            episodes: (13..=24).map(|n| view(1, n)).collect(),
        };
        assert_eq!(w.link("[G] X - 01 [1080p].mkv").map(|e| e.number), Some(13));
        assert!(w.link("ncop.mkv").is_none());
    }

    #[test]
    fn cache_key_is_stable() {
        let q = |titles: &[&str]| WorkQuery {
            titles: titles.iter().map(|t| (*t).into()).collect(),
            year: Some(2014),
            start: None,
            kind: WorkKind::Series,
        };
        assert_eq!(work_key(&q(&["Happiness Charge Precure!"])), work_key(&q(&["HappinessCharge PRECURE"])));
        assert_ne!(work_key(&q(&["A"])), work_key(&q(&["B"])));
        let mut special = q(&["A"]);
        special.kind = WorkKind::Special;
        assert_ne!(work_key(&q(&["A"])), work_key(&special));
    }

    #[tokio::test]
    #[ignore = "usa a rede e credenciais do build"]
    async fn live_happiness_charge() {
        let dir = std::env::temp_dir().join("higanbana-tmdb-live");
        let tmdb = Tmdb::new(&dir);
        let q = WorkQuery {
            titles: vec!["HappinessCharge Precure!".into()],
            year: Some(2014),
            start: None,
            kind: WorkKind::Series,
        };
        let w = tmdb.work_episodes(&q).await.unwrap().expect("obra");
        assert_eq!(w.episodes.len(), 49);
        let first = w.episodes.iter().find(|e| e.number == 1).unwrap();
        assert!(first.title.is_ascii(), "título: {}", first.title);
    }
}
