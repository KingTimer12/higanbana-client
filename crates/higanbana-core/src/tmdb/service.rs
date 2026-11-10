//! Episódios de uma obra pelo TMDB, com cache em disco. Erro de rede nunca vai para o cache.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    TmdbError, TmdbResult,
    client::TmdbClient,
    matching::{
        Anchor, WorkKind, WorkQuery, foreign_seasons, map_file, pick_anchor, pick_movie, pick_overview, pick_text,
        rank_shows, season_numbered,
    },
    types::{Episode, TvShow},
};
use crate::{
    anilist::cache::DiskCache,
    player::logic::{episode_number, episode_season},
};

const PT: &str = "pt-BR";
const EN: &str = "en-US";
/// Obra casada (série e âncora) fica uma semana; "não tem" só um dia, para uma
/// série recém-cadastrada no TMDB aparecer logo.
const WORKS_TTL: i64 = 7 * 24 * 3600;
const DAY_TTL: i64 = 24 * 3600;
/// Séries candidatas conferidas pela data, por grupo de títulos (da obra; das ligadas).
const MAX_CANDIDATES: usize = 3;

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
    /// Obra de um episódio só (filme, especial único): todo vídeo da pasta é ele.
    #[serde(default)]
    pub single: bool,
}

impl WorkEpisodes {
    /// Episódio do TMDB de cada arquivo de uma pasta, na mesma ordem. A pasta decide
    /// uma vez se segue a numeração da temporada (`season_numbered`), contando só os
    /// arquivos sem temporada no nome ou com a temporada da âncora. Temporada do
    /// nome que não bate com a do TMDB (`foreign_seasons`) não liga nenhum arquivo.
    /// Obra de um episódio só: todos os vídeos são esse episódio, com ou sem número.
    pub fn links(&self, file_names: &[&str]) -> Vec<Option<&EpisodeView>> {
        let anchor = self.anchor;
        if self.single {
            let only = self.episodes.iter().find(|e| e.season == anchor.season && e.number == anchor.offset + 1);
            return vec![only; file_names.len()];
        }
        let parsed: Vec<(Option<u32>, Option<u32>)> =
            file_names.iter().map(|f| (episode_season(f), episode_number(f))).collect();
        let foreign = foreign_seasons(&self.show, parsed.iter().filter_map(|&(s, n)| Some((s?, n?))));
        let mode = season_numbered(
            anchor,
            parsed.iter().filter(|(s, _)| s.is_none_or(|s| s == anchor.season)).filter_map(|&(_, n)| n),
        );
        parsed
            .into_iter()
            .map(|(season, number)| {
                if season.is_some_and(|s| foreign.contains(&s)) {
                    return None;
                }
                let (season, n) = map_file(anchor, &self.show, season, number?, mode)?;
                self.episodes.iter().find(|e| e.season == season && e.number == n)
            })
            .collect()
    }
}

fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Chave de cache da obra (e do mapa no desktop): títulos normalizados, ano, início
/// (só ano e mês quando o dia não é conhecido) e tipo.
pub fn work_key(q: &WorkQuery) -> String {
    let titles: Vec<String> = q.titles.iter().map(|t| normalize(t)).filter(|t| !t.is_empty()).collect();
    let kind = match q.kind {
        WorkKind::Series => "series",
        WorkKind::Special => "special",
        WorkKind::Movie => "movie",
    };
    let year = q.year.map(|y| y.to_string()).unwrap_or_default();
    let start = match q.start {
        Some(s) if q.start_day_known => format!("{}-{:02}-{:02}", s.y, s.m, s.d),
        Some(s) => format!("{}-{:02}", s.y, s.m),
        None => String::new(),
    };
    // `v2`: regras novas (filmes, obras ligadas) não herdam os "não tem" antigos.
    format!("{}|{year}|{start}|{kind}|v2", titles.join("|"))
}

pub struct Tmdb {
    client: TmdbClient,
    works: DiskCache<(u64, Anchor)>,
    /// Obras casadas com um filme do TMDB: id e o filme já como episódio único.
    movies: DiskCache<(u64, EpisodeView)>,
    /// Obras sem série ou sem âncora no TMDB (`DAY_TTL`).
    misses: DiskCache<bool>,
    shows: DiskCache<TvShow>,
    seasons: DiskCache<Vec<Episode>>,
}

impl Tmdb {
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            client: TmdbClient::new(),
            works: DiskCache::open(cache_dir.join("tmdb-works.json"), WORKS_TTL),
            movies: DiskCache::open(cache_dir.join("tmdb-movies.json"), WORKS_TTL),
            misses: DiskCache::open(cache_dir.join("tmdb-works-miss.json"), DAY_TTL),
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
        self.movies.flush();
        self.misses.flush();
        self.shows.flush();
        self.seasons.flush();
        result
    }

    async fn resolve(&self, key: &str, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>> {
        if let Some((id, view)) = self.movies.get(key) {
            return Ok(Some(movie_work(id, view)));
        }
        match self.cached(key) {
            Some(None) => return Ok(None),
            Some(Some((id, anchor))) => return self.tv_work(id, Some(anchor), q).await,
            None => {}
        }
        // Filme: `/movie`; se não houver, a temporada 0 de uma série (o TMDB guarda
        // alguns como especial). Especial único: o caminho inverso.
        let found = match q.kind {
            WorkKind::Movie => match self.find_movie(key, q).await? {
                Some(w) => Some(w),
                None => self.find_tv(key, q).await?,
            },
            WorkKind::Special => match self.find_tv(key, q).await? {
                Some(w) => Some(w),
                None if q.single => self.find_movie(key, q).await?,
                None => None,
            },
            WorkKind::Series => self.find_tv(key, q).await?,
        };
        if found.is_none() {
            self.remember(key, None);
        }
        Ok(found)
    }

    /// Série e âncora da obra: as candidatas pelo título da obra e, com data de
    /// início, pelas obras ligadas; a primeira em que a âncora fecha vence.
    async fn find_tv(&self, key: &str, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>> {
        // Sem data a âncora não confere nada: só a primeira candidata, como sempre.
        let limit = if q.start.is_some() { MAX_CANDIDATES } else { 1 };
        let mut ids = self.candidates(&q.titles, q.year, limit).await?;
        if q.start.is_some() {
            for id in self.candidates(&q.related, q.year, MAX_CANDIDATES).await? {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        for id in ids {
            if let Some(work) = self.tv_work(id, None, q).await? {
                self.works.insert(key.to_owned(), (id, work.anchor));
                return Ok(Some(work));
            }
        }
        Ok(None)
    }

    /// Candidatas da primeira busca que casar algum título.
    async fn candidates(&self, titles: &[String], year: Option<i32>, limit: usize) -> TmdbResult<Vec<u64>> {
        for title in titles.iter().filter(|t| !t.trim().is_empty()) {
            let hits = self.client.search_tv(title, PT).await?;
            let mut ids = rank_shows(&hits, titles, year);
            if !ids.is_empty() {
                ids.truncate(limit);
                return Ok(ids);
            }
        }
        Ok(Vec::new())
    }

    /// Episódios da série `show_id` para a obra; `None` quando a âncora não fecha.
    async fn tv_work(&self, show_id: u64, known: Option<Anchor>, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>> {
        let show = self.show(show_id).await?;
        let mut pt = Vec::new();
        for season in wanted_seasons(&show, q.kind) {
            pt.push((season, self.season(show_id, season, PT).await?));
        }
        let Some(anchor) = known.or_else(|| pick_anchor(&show, &pt, q)) else { return Ok(None) };
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
        Ok(Some(WorkEpisodes {
            show_id,
            show_vote: (show.vote_average, show.vote_count),
            anchor,
            show,
            episodes,
            single: q.single,
        }))
    }

    /// Filme do TMDB como obra de um episódio só (a imagem de fundo faz de miniatura).
    async fn find_movie(&self, key: &str, q: &WorkQuery) -> TmdbResult<Option<WorkEpisodes>> {
        for title in q.titles.iter().filter(|t| !t.trim().is_empty()) {
            let hits = self.client.search_movie(title, PT).await?;
            let Some(id) = pick_movie(&hits, q) else { continue };
            let pt = self.client.movie(id, PT).await?;
            let en_overview = if pt.overview.trim().is_empty() {
                self.client.movie(id, EN).await?.overview
            } else {
                String::new()
            };
            let view = EpisodeView {
                season: 0,
                number: 1,
                title: pt.title.trim().to_owned(),
                overview: pick_overview(&pt.overview, &en_overview),
                air_date: pt.release_date.filter(|d| !d.is_empty()),
                still_path: pt.backdrop_path.filter(|p| !p.is_empty()),
                vote_average: pt.vote_average,
                vote_count: pt.vote_count,
                runtime: pt.runtime.filter(|&m| m > 0),
            };
            self.movies.insert(key.to_owned(), (id, view.clone()));
            return Ok(Some(movie_work(id, view)));
        }
        Ok(None)
    }

    /// Casamento guardado: `Some(Some(..))` série e âncora; `Some(None)` o TMDB não
    /// tem a obra; `None` nada guardado (ou vencido).
    fn cached(&self, key: &str) -> Option<Option<(u64, Anchor)>> {
        if let Some(hit) = self.works.get(key) {
            return Some(Some(hit));
        }
        self.misses.get(key).map(|_| None)
    }

    /// Guarda o casamento: achou → uma semana; não achou → um dia.
    fn remember(&self, key: &str, found: Option<(u64, Anchor)>) {
        match found {
            Some(hit) => self.works.insert(key.to_owned(), hit),
            None => self.misses.insert(key.to_owned(), true),
        }
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

/// O filme como obra: um episódio só, na "temporada 0", que todo vídeo da pasta usa.
fn movie_work(id: u64, view: EpisodeView) -> WorkEpisodes {
    WorkEpisodes {
        show_id: id,
        show_vote: (view.vote_average, view.vote_count),
        anchor: Anchor { season: 0, offset: 0 },
        show: TvShow {
            id,
            name: view.title.clone(),
            vote_average: view.vote_average,
            vote_count: view.vote_count,
            seasons: Vec::new(),
        },
        episodes: vec![view],
        single: true,
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
            WorkKind::Special | WorkKind::Movie => s == 0,
        })
        .collect();
    seasons.sort_unstable();
    seasons.dedup();
    seasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmdb::{matching::Ymd, types::SeasonInfo};

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
            single: false,
        };
        let first = |names: &[&str]| w.links(names)[0].map(|e| e.number);
        assert_eq!(first(&["[G] X - 01 [1080p].mkv", "[G] X - 02 [1080p].mkv"]), Some(13));
        assert_eq!(first(&["[G] X - 13 [1080p].mkv", "[G] X - 14 [1080p].mkv"]), Some(13));
        let both: Vec<Option<u32>> = w.links(&["ncop.mkv", "[G] X - 02.mkv"]).iter().map(|e| e.map(|e| e.number)).collect();
        assert_eq!(both, [None, Some(14)]);
    }

    #[test]
    fn links_decide_numbering_per_folder() {
        let show = TvShow {
            id: 1,
            name: "X".into(),
            vote_average: 0.0,
            vote_count: 0,
            seasons: vec![SeasonInfo { season_number: 1, episode_count: 25, air_date: None }],
        };
        let w = WorkEpisodes {
            show_id: 1,
            show_vote: (0.0, 0),
            anchor: Anchor { season: 1, offset: 12 },
            show,
            episodes: (1..=25).map(|n| view(1, n)).collect(),
            single: false,
        };
        let numbers = |names: Vec<String>| {
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            w.links(&refs).iter().map(|e| e.map(|e| e.number)).collect::<Vec<_>>()
        };
        let all: Vec<Option<u32>> = (13..=25).map(Some).collect();
        assert_eq!(numbers((13..=25).map(|n| format!("X - {n:02}.mkv")).collect()), all);
        assert_eq!(numbers((1..=13).map(|n| format!("X - {n:02}.mkv")).collect()), all);
    }

    #[test]
    fn single_work_links_every_video() {
        let show = TvShow {
            id: 1,
            name: "X".into(),
            vote_average: 0.0,
            vote_count: 0,
            seasons: vec![SeasonInfo { season_number: 0, episode_count: 9, air_date: None }],
        };
        let w = WorkEpisodes {
            show_id: 1,
            show_vote: (0.0, 0),
            anchor: Anchor { season: 0, offset: 6 },
            show,
            episodes: (1..=9).map(|n| view(0, n)).collect(),
            single: true,
        };
        let got: Vec<Option<u32>> =
            w.links(&["[G] X - Episódio da Nami [720p].mkv", "Filme 02 [BD].mkv"]).iter().map(|e| e.map(|e| e.number)).collect();
        assert_eq!(got, [Some(7), Some(7)]);
        // Filme: um episódio só, qualquer nome de arquivo.
        let m = movie_work(5, view(0, 1));
        assert_eq!(m.links(&["One.Piece.Film.Red.2022.1080p.mkv"])[0].map(|e| e.number), Some(1));
        assert!(m.single);
    }

    #[test]
    fn foreign_season_split_links_nothing() {
        let show = TvShow {
            id: 1,
            name: "X".into(),
            vote_average: 0.0,
            vote_count: 0,
            seasons: [(1, 61), (2, 16), (3, 14)]
                .map(|(season_number, episode_count)| SeasonInfo { season_number, episode_count, air_date: None })
                .to_vec(),
        };
        let w = WorkEpisodes {
            show_id: 1,
            show_vote: (0.0, 0),
            anchor: Anchor { season: 1, offset: 0 },
            show,
            episodes: (1..=14).map(|n| view(3, n)).chain((1..=16).map(|n| view(2, n))).collect(),
            single: false,
        };
        let numbers = |names: Vec<String>| {
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            w.links(&refs).iter().map(|e| e.map(|e| (e.season, e.number))).collect::<Vec<_>>()
        };
        // 15 arquivos numa temporada que no TMDB tem 14: outra divisão, nada liga.
        assert!(numbers((1..=15).map(|n| format!("X.S03E{n:02}.mkv")).collect()).iter().all(Option::is_none));
        // Temporada que bate continua ligando.
        assert_eq!(numbers(vec!["X.S02E16.mkv".into()]), [Some((2, 16))]);
    }

    #[test]
    fn cache_key_is_stable() {
        let q = |titles: &[&str]| WorkQuery {
            titles: titles.iter().map(|t| (*t).into()).collect(),
            year: Some(2014),
            start: None,
            start_day_known: false,
            kind: WorkKind::Series,
            related: Vec::new(),
            single: false,
        };
        assert_eq!(work_key(&q(&["Happiness Charge Precure!"])), work_key(&q(&["HappinessCharge PRECURE"])));
        assert_ne!(work_key(&q(&["A"])), work_key(&q(&["B"])));
        let mut special = q(&["A"]);
        special.kind = WorkKind::Special;
        assert_ne!(work_key(&q(&["A"])), work_key(&special));
        // Início sem o dia não divide a chave com o dia 1 de verdade.
        let mut day_one = q(&["A"]);
        day_one.start = Some(Ymd { y: 2014, m: 2, d: 1 });
        day_one.start_day_known = true;
        let mut no_day = day_one.clone();
        no_day.start_day_known = false;
        assert_ne!(work_key(&day_one), work_key(&no_day));
        assert!(work_key(&no_day).contains("|2014-02|"));
        let mut movie = q(&["A"]);
        movie.kind = WorkKind::Movie;
        assert_ne!(work_key(&movie), work_key(&special));
    }

    #[test]
    fn misses_expire_in_a_day_matches_in_a_week() {
        let dir = std::env::temp_dir().join(format!("higanbana-tmdb-ttl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let (two_hours, two_days) = (now - 2 * 3600, now - 2 * 24 * 3600);
        std::fs::write(
            dir.join("tmdb-works.json"),
            format!(r#"{{"hit":{{"at":{two_days},"value":[5,{{"season":1,"offset":0}}]}}}}"#),
        )
        .unwrap();
        std::fs::write(
            dir.join("tmdb-works-miss.json"),
            format!(r#"{{"old":{{"at":{two_days},"value":true}},"new":{{"at":{two_hours},"value":true}}}}"#),
        )
        .unwrap();
        let tmdb = Tmdb::new(&dir);
        assert_eq!(tmdb.cached("hit"), Some(Some((5, Anchor { season: 1, offset: 0 }))));
        assert_eq!(tmdb.cached("new"), Some(None));
        assert_eq!(tmdb.cached("old"), None);
        assert_eq!(tmdb.cached("nada"), None);
        // Gravar: casamento no cache de 7 dias, ausência no de 24 h.
        tmdb.remember("a", None);
        tmdb.remember("b", Some((9, Anchor { season: 2, offset: 3 })));
        assert_eq!(tmdb.cached("a"), Some(None));
        assert!(tmdb.works.get("a").is_none());
        assert_eq!(tmdb.cached("b"), Some(Some((9, Anchor { season: 2, offset: 3 }))));
        assert!(tmdb.misses.get("b").is_none());
        let _ = std::fs::remove_dir_all(&dir);
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
            start_day_known: false,
            kind: WorkKind::Series,
            related: Vec::new(),
            single: false,
        };
        let w = tmdb.work_episodes(&q).await.unwrap().expect("obra");
        assert_eq!(w.episodes.len(), 49);
        let first = w.episodes.iter().find(|e| e.number == 1).unwrap();
        assert!(first.title.is_ascii(), "título: {}", first.title);
    }

    /// Rede real (só TMDB). `cargo test -p higanbana-core -- --ignored live_related_single_movie --nocapture`.
    #[tokio::test]
    #[ignore = "usa a rede e credenciais do build"]
    async fn live_related_single_movie() {
        let dir = std::env::temp_dir().join(format!("higanbana-tmdb-live2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tmdb = Tmdb::new(&dir);
        let q = |titles: &[&str], related: &[&str], start: &str, kind: WorkKind, single: bool| WorkQuery {
            titles: titles.iter().map(|t| (*t).into()).collect(),
            year: Ymd::parse(start).map(|d| d.y),
            start: Ymd::parse(start),
            start_day_known: true,
            kind,
            related: related.iter().map(|t| (*t).into()).collect(),
            single,
        };
        // Continuação que o TMDB guarda como 2ª temporada da série-mãe.
        let w = tmdb
            .work_episodes(&q(&["Futari wa Precure: Max Heart"], &["ふたりはプリキュア", "Futari wa Precure"], "2005-02-06", WorkKind::Series, false))
            .await
            .unwrap()
            .expect("max heart");
        println!("max heart: show {} anchor {:?}", w.show_id, w.anchor);
        assert_eq!(w.anchor, Anchor { season: 2, offset: 0 });
        // Especial único sem número no arquivo, achado pela série-mãe.
        let w = tmdb
            .work_episodes(&q(&["One Piece: Episode of Nami - Koukaishi no Namida to Nakama no Kizuna"], &["ONE PIECE", "One Piece"], "2012-08-25", WorkKind::Special, true))
            .await
            .unwrap()
            .expect("episode of nami");
        let ep = w.links(&["[Kyoshiro&OPex] One Piece - Episódio da Nami [HDTV] [720p].mkv"])[0].expect("ligado");
        println!("nami: S{}E{} {} still={}", ep.season, ep.number, ep.title, ep.still_path.is_some());
        assert_eq!((ep.season, ep.number), (0, 7));
        // Especial de 2024: a série live-action de 2023 é descartada pela data.
        let w = tmdb
            .work_episodes(&q(&["ONE PIECE FAN LETTER"], &["ONE PIECE", "One Piece"], "2024-10-20", WorkKind::Special, true))
            .await
            .unwrap()
            .expect("fan letter");
        let ep = w.links(&["[Erai-raws] One Piece - Fan Letter [1080p].mkv"])[0].expect("ligado");
        println!("fan letter: show {} S{}E{} {}", w.show_id, ep.season, ep.number, ep.title);
        assert_eq!(w.show_id, 37854);
        // Filme.
        let w = tmdb
            .work_episodes(&q(&["One Piece Film: Red", "ONE PIECE FILM RED"], &[], "2022-08-06", WorkKind::Movie, true))
            .await
            .unwrap()
            .expect("film red");
        let ep = w.links(&["One.Piece.Film.Red.2022.1080p.AMZN.WEB-DL.mkv"])[0].expect("ligado");
        println!("red: {} | {:?} min | still={} | votos={}", ep.title, ep.runtime, ep.still_path.is_some(), ep.vote_count);
        assert!(ep.still_path.is_some());
        let w = tmdb
            .work_episodes(&q(&["Eiga Futari wa Precure: Max Heart", "映画 ふたりはプリキュア Max Heart"], &[], "2005-04-16", WorkKind::Movie, true))
            .await
            .unwrap();
        println!("precure filme: {:?}", w.map(|w| (w.show_id, w.episodes[0].title.clone(), w.episodes[0].still_path.is_some())));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
