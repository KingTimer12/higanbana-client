//! Casamento puro obra/arquivo → episódio do TMDB. Na dúvida, não liga nada.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::types::{Episode, TvHit, TvShow};
use crate::{anilist::types::MediaSummary, library::parse_work_name};

/// Tolerância, em dias, entre o início da obra no AniList e a `air_date` do TMDB.
const DATE_TOLERANCE: i64 = 7;

/// Data `AAAA-MM-DD` como vem do TMDB e do AniList.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ymd {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

impl Ymd {
    /// `"2023-09-29"` → data; vazio, lixo ou data impossível → `None`.
    pub fn parse(s: &str) -> Option<Ymd> {
        let mut parts = s.trim().split('-');
        let y = parts.next()?.parse().ok()?;
        let m = parts.next()?.parse().ok()?;
        let d = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
            return None;
        }
        Some(Ymd { y, m, d })
    }

    /// Dias desde 1970-01-01 (days-from-civil de Howard Hinnant).
    pub fn days(&self) -> i64 {
        let m = i64::from(self.m);
        let y = i64::from(self.y) - i64::from(m <= 2);
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn parse_date(s: Option<&str>) -> Option<Ymd> {
    s.and_then(Ymd::parse)
}

/// Tipo da obra. ONA/TV/TV_SHORT → `Series`; OVA/SPECIAL → `Special`. MOVIE fica fora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkKind {
    Series,
    Special,
}

/// O que se sabe da obra (AniList/pasta) para achar a série e a temporada no TMDB.
#[derive(Debug, Clone)]
pub struct WorkQuery {
    pub titles: Vec<String>,
    pub year: Option<i32>,
    pub start: Option<Ymd>,
    /// O AniList deu o dia de `start`? Sem ele (`d` = 1 por convenção), a âncora
    /// tem que estrear no mesmo ano e mês, e não a ±7 dias do dia 1.
    pub start_day_known: bool,
    pub kind: WorkKind,
}

/// Arquivo `n` → episódio `offset + n` da `season`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub season: u32,
    pub offset: u32,
}

/// Minúsculas e só alfanuméricos: "Sousou no Frieren!" → "sousounofrieren".
fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Série do TMDB da obra: o título tem que casar (igualdade exata primeiro; só sem
/// nenhuma igual, nome que contém o título); dentro desse grupo, o ano de estreia
/// igual ao da obra; senão a estreia mais recente antes dele (parte posterior →
/// série-mãe). Sem ano, o primeiro do grupo.
pub fn pick_show(hits: &[TvHit], q: &WorkQuery) -> Option<u64> {
    let titles: Vec<String> = q.titles.iter().map(|t| normalize(t)).filter(|t| !t.is_empty()).collect();
    let matching = |exact: bool| -> Vec<&TvHit> {
        hits.iter()
            .filter(|h| {
                [&h.name, &h.original_name].into_iter().map(|n| normalize(n)).any(|n| {
                    !n.is_empty() && titles.iter().any(|t| if exact { n == *t } else { n.contains(t.as_str()) })
                })
            })
            .collect()
    };
    let mut found = matching(true);
    if found.is_empty() {
        found = matching(false);
    }
    let Some(year) = q.year else {
        return found.first().map(|h| h.id);
    };
    let premiere = |h: &TvHit| parse_date(h.first_air_date.as_deref());
    if let Some(h) = found.iter().find(|h| premiere(h).is_some_and(|d| d.y == year)) {
        return Some(h.id);
    }
    found
        .iter()
        .filter_map(|h| premiere(h).filter(|d| d.y < year).map(|d| (d.days(), h.id)))
        .max_by_key(|&(days, _)| days)
        .map(|(_, id)| id)
}

/// Episódio que estreou mais perto de `start`, a até `DATE_TOLERANCE` dias (sem o
/// dia, `day_known` falso: no mesmo ano e mês de `start`). Em séries semanais o
/// vizinho fica a 7 dias, então vence o mais próximo. Empatados na mesma temporada
/// e no mesmo dia (estreia dupla, temporada inteira de uma vez) → o menor número;
/// empate entre datas ou temporadas diferentes é dúvida → `None`.
fn anchor_near<'a>(
    seasons: impl Iterator<Item = &'a (u32, Vec<Episode>)>,
    start: Ymd,
    day_known: bool,
) -> Option<Anchor> {
    let mut near: Vec<(i64, u32, i64, u32)> = seasons
        .flat_map(|(s, list)| list.iter().map(move |e| (*s, e)))
        .filter(|(_, e)| e.episode_number > 0)
        .filter_map(|(season, e)| {
            let date = parse_date(e.air_date.as_deref())?;
            let day = date.days();
            let diff = (day - start.days()).abs();
            let close = if day_known { diff <= DATE_TOLERANCE } else { date.y == start.y && date.m == start.m };
            close.then_some((diff, season, day, e.episode_number))
        })
        .collect();
    near.sort_unstable();
    let &(best, season, day, number) = near.first()?;
    if near.iter().take_while(|n| n.0 == best).any(|n| n.1 != season || n.2 != day) {
        return None;
    }
    Some(Anchor { season, offset: number - 1 })
}

/// Onde a obra começa dentro da série. `Series`: temporadas regulares, pelo
/// episódio que estreou junto com a obra; sem data, só se houver uma única
/// temporada regular. `Special`: só a temporada 0, e só com data.
pub fn pick_anchor(show: &TvShow, seasons: &[(u32, Vec<Episode>)], q: &WorkQuery) -> Option<Anchor> {
    match q.kind {
        WorkKind::Series => {
            if let Some(start) = q.start {
                return anchor_near(seasons.iter().filter(|(s, _)| *s > 0), start, q.start_day_known);
            }
            let regular: Vec<u32> = show.seasons.iter().map(|s| s.season_number).filter(|&s| s > 0).collect();
            match regular[..] {
                [season] => Some(Anchor { season, offset: 0 }),
                _ => None,
            }
        }
        WorkKind::Special => anchor_near(seasons.iter().filter(|(s, _)| *s == 0), q.start?, q.start_day_known),
    }
}

/// A pasta segue a numeração da própria temporada? Só com âncora numa temporada
/// regular e deslocamento > 0: sim quando o menor número entre os arquivos da
/// pasta passa do deslocamento (ex. 13–25 numa obra que começa no 13); senão a
/// pasta é relativa à obra (1, 2, … → 13, 14, …). Decidido uma vez por pasta,
/// nunca arquivo a arquivo: numa temporada de 25 com a obra a partir do 13, o
/// arquivo "13" é o ep. 13 numa pasta 13–25 e o ep. 25 numa pasta 1–13.
pub fn season_numbered(anchor: Anchor, numbers: impl IntoIterator<Item = u32>) -> bool {
    anchor.season > 0
        && anchor.offset > 0
        && numbers.into_iter().filter(|&n| n > 0).min().is_some_and(|min| min > anchor.offset)
}

/// Arquivo de número `number` → `(temporada, episódio)` no TMDB. Regras, em ordem:
/// 1. temporada explícita no nome (`S02E03`) diferente da âncora e que existe no
///    TMDB vence; se o número não cabe nela, não liga (dúvida). Igual à âncora
///    (`S01E01` numa obra que começa no ep. 13 da S1) segue as regras abaixo. Com
///    âncora na temporada 0, só `S00` vale aqui;
/// 2. pasta numerada pela temporada (`season_numbered`, decidido para a pasta
///    inteira) → `(âncora, number)` se couber na temporada;
/// 3. senão, `offset + number <= total` → `(âncora, offset + number)`;
/// 4. só com `offset == 0` numa temporada regular: numeração absoluta, somando os
///    totais das temporadas regulares a partir da 1 (caso One Piece), nunca
///    caindo numa temporada anterior à da âncora;
/// 5. nada disso → `None`.
pub fn map_file(
    anchor: Anchor,
    show: &TvShow,
    file_season: Option<u32>,
    number: u32,
    season_numbered: bool,
) -> Option<(u32, u32)> {
    if number == 0 {
        return None;
    }
    let count = |season: u32| show.seasons.iter().find(|s| s.season_number == season).map(|s| s.episode_count);
    if let Some(season) = file_season
        && season != anchor.season
        && (anchor.season > 0 || season == 0)
        && let Some(total) = count(season) {
            return (number <= total).then_some((season, number));
        }
    let total = count(anchor.season)?;
    if season_numbered && anchor.season > 0 && anchor.offset > 0 {
        return (number <= total).then_some((anchor.season, number));
    }
    if let Some(ep) = anchor.offset.checked_add(number)
        && ep <= total {
            return Some((anchor.season, ep));
        }
    if anchor.offset != 0 || anchor.season == 0 {
        return None;
    }
    let mut regular: Vec<(u32, u32)> =
        show.seasons.iter().filter(|s| s.season_number > 0).map(|s| (s.season_number, s.episode_count)).collect();
    regular.sort_by_key(|&(s, _)| s);
    let mut rest = number;
    for (season, total) in regular {
        if rest <= total {
            return (season >= anchor.season).then_some((season, rest));
        }
        rest -= total;
    }
    None
}

static GENERIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:epis[oó]dio|episode|ep\.?)\s*\d+$|^第\d+話$").unwrap());

/// Título que só repete o número ("Episódio 3", "Episode 12").
pub fn is_generic_title(s: &str) -> bool {
    GENERIC.is_match(s.trim())
}

/// Texto pt-BR; se vazio ou genérico, o en-US; senão "Episódio N".
pub fn pick_text(pt: &str, en: &str, n: u32) -> String {
    [pt, en]
        .into_iter()
        .map(str::trim)
        .find(|t| !t.is_empty() && !is_generic_title(t))
        .map_or_else(|| format!("Episódio {n}"), str::to_owned)
}

/// Sinopse pt-BR; se vazia, a en-US; senão vazia (nunca "Episódio N").
pub fn pick_overview(pt: &str, en: &str) -> String {
    [pt, en].into_iter().map(str::trim).find(|t| !t.is_empty()).unwrap_or_default().to_owned()
}

/// O que pedir ao TMDB sobre a obra: títulos do AniList e da pasta, ano, data de
/// início (sem dia → dia 1 e `start_day_known` falso; sem mês → nenhuma) e tipo. Filme (e música) fica fora.
pub fn work_query(detail: &MediaSummary, folder_name: &str) -> Option<WorkQuery> {
    let kind = match detail.format.as_deref() {
        Some("MOVIE" | "MUSIC") => return None,
        Some("OVA" | "SPECIAL") => WorkKind::Special,
        _ => WorkKind::Series,
    };
    let t = &detail.title;
    let folder = parse_work_name(folder_name).title;
    let mut titles: Vec<String> = Vec::new();
    let candidates = [t.romaji.as_deref(), t.english.as_deref(), t.native.as_deref()]
        .into_iter()
        .flatten()
        .chain(detail.synonyms.iter().map(String::as_str))
        .chain(std::iter::once(folder.as_str()));
    for c in candidates.map(str::trim).filter(|c| !c.is_empty()) {
        if !titles.iter().any(|t| t == c) {
            titles.push(c.to_owned());
        }
    }
    let start = detail.start_date.and_then(|d| Some(Ymd { y: d.year?, m: d.month?, d: d.day.unwrap_or(1) }));
    let start_day_known = detail.start_date.is_some_and(|d| d.day.is_some());
    Some(WorkQuery { titles, year: detail.year(), start, start_day_known, kind })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmdb::types::SeasonInfo;

    fn ymd(s: &str) -> Ymd {
        Ymd::parse(s).unwrap()
    }

    fn hit(id: u64, name: &str, original: &str, date: &str) -> TvHit {
        TvHit { id, name: name.into(), original_name: original.into(), first_air_date: Some(date.into()) }
    }

    fn show(seasons: &[(u32, u32)]) -> TvShow {
        TvShow {
            id: 1,
            name: "X".into(),
            vote_average: 0.0,
            vote_count: 0,
            seasons: seasons
                .iter()
                .map(|&(season_number, episode_count)| SeasonInfo { season_number, episode_count, air_date: None })
                .collect(),
        }
    }

    fn ep(season: u32, number: u32, date: Option<&str>) -> Episode {
        Episode {
            season_number: season,
            episode_number: number,
            name: String::new(),
            overview: String::new(),
            air_date: date.map(Into::into),
            still_path: None,
            vote_average: 0.0,
            vote_count: 0,
            runtime: None,
        }
    }

    fn query(titles: &[&str], year: Option<i32>, start: Option<&str>, kind: WorkKind) -> WorkQuery {
        WorkQuery {
            titles: titles.iter().map(|t| (*t).into()).collect(),
            year,
            start: start.map(ymd),
            start_day_known: true,
            kind,
        }
    }

    #[test]
    fn ymd_parses_and_counts() {
        assert_eq!(Ymd::parse("2023-09-29"), Some(Ymd { y: 2023, m: 9, d: 29 }));
        assert_eq!(ymd("2023-10-06").days() - ymd("2023-09-29").days(), 7);
        assert_eq!(ymd("1970-01-01").days(), 0);
        assert_eq!(ymd("2024-03-01").days() - ymd("2024-02-28").days(), 2);
        assert_eq!(Ymd::parse(""), None);
        assert_eq!(Ymd::parse("x"), None);
        assert_eq!(Ymd::parse("2023-13-01"), None);
    }

    #[test]
    fn show_by_title_and_year() {
        let hits = [
            hit(1, "Frieren Fan Show", "", "2019-01-01"),
            hit(209867, "Frieren e a Jornada para o Além", "葬送のフリーレン", "2023-09-29"),
            hit(5, "Outra coisa", "", "2023-01-01"),
        ];
        let q = query(&["Sousou no Frieren", "Frieren"], Some(2023), None, WorkKind::Series);
        assert_eq!(pick_show(&hits, &q), Some(209867));
    }

    #[test]
    fn later_part_picks_parent() {
        let hits = [hit(7, "Spy x Family", "SPY×FAMILY", "2022-04-09"), hit(8, "Spy x Family Code", "", "2024-01-01")];
        let q = query(&["Spy x Family"], Some(2023), None, WorkKind::Series);
        assert_eq!(pick_show(&hits, &q), Some(7));
        let q = query(&["Spy x Family"], None, None, WorkKind::Series);
        assert_eq!(pick_show(&hits, &q), Some(7));
        let q = query(&["Spy x Family"], Some(2021), None, WorkKind::Series);
        assert_eq!(pick_show(&hits, &q), None);
    }

    #[test]
    fn no_title_no_show() {
        let hits = [hit(1, "Frieren", "", "2023-09-29")];
        assert_eq!(pick_show(&hits, &query(&["Naruto"], Some(2023), None, WorkKind::Series)), None);
        assert_eq!(pick_show(&hits, &query(&["!!!", ""], Some(2023), None, WorkKind::Series)), None);
    }

    #[test]
    fn single_season_anchor() {
        let s = show(&[(0, 2), (1, 12)]);
        let eps = [(1, vec![ep(1, 1, Some("2020-01-01"))])];
        let q = query(&["X"], None, None, WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 0 }));
    }

    #[test]
    fn split_cour_anchor() {
        let s = show(&[(1, 24)]);
        let mut list: Vec<Episode> = (1..=24).map(|n| ep(1, n, None)).collect();
        list[0].air_date = Some("2022-04-09".into());
        list[12].air_date = Some("2022-10-02".into());
        let eps = [(1, list)];
        let q = query(&["X"], Some(2022), Some("2022-10-01"), WorkKind::Series);
        let a = pick_anchor(&s, &eps, &q).unwrap();
        assert_eq!(a, Anchor { season: 1, offset: 12 });
        assert_eq!(map_file(a, &s, None, 1, false), Some((1, 13)));
        assert_eq!(map_file(a, &s, None, 13, true), Some((1, 13)));
        assert_eq!(map_file(a, &s, None, 25, false), None);
        assert_eq!(map_file(a, &s, None, 0, false), None);
    }

    #[test]
    fn start_mismatch_no_anchor() {
        let s = show(&[(1, 12), (2, 12)]);
        let eps = [
            (1, vec![ep(1, 1, Some("2020-01-01")), ep(1, 2, Some(""))]),
            (2, vec![ep(2, 1, Some("2021-01-01")), ep(2, 2, None)]),
        ];
        let q = query(&["X"], Some(2023), Some("2023-04-01"), WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), None);
        let q = query(&["X"], Some(2023), None, WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), None);
    }

    #[test]
    fn absolute_numbering() {
        let s = show(&[(0, 5), (1, 61), (2, 16), (3, 14)]);
        let a = Anchor { season: 1, offset: 0 };
        assert_eq!(map_file(a, &s, None, 5, false), Some((1, 5)));
        assert_eq!(map_file(a, &s, None, 70, false), Some((2, 9)));
        assert_eq!(map_file(a, &s, None, 91, false), Some((3, 14)));
        assert_eq!(map_file(a, &s, None, 92, false), None);
    }

    #[test]
    fn explicit_season_wins() {
        let s = show(&[(1, 61), (2, 16)]);
        let a = Anchor { season: 1, offset: 0 };
        assert_eq!(map_file(a, &s, Some(2), 3, false), Some((2, 3)));
        assert_eq!(map_file(a, &s, Some(9), 3, false), Some((1, 3)));
        // Temporada explícita existe mas o número não cabe: dúvida, não liga.
        assert_eq!(map_file(a, &s, Some(2), 20, false), None);
    }

    #[test]
    fn special_anchor_season_zero() {
        let s = show(&[(0, 6), (1, 12)]);
        let eps = [
            (0, vec![ep(0, 4, Some("2014-01-01")), ep(0, 5, Some("2015-03-01")), ep(0, 6, Some("2016-01-01"))]),
            (1, vec![ep(1, 1, Some("2015-03-02"))]),
        ];
        let q = query(&["X"], Some(2015), Some("2015-03-03"), WorkKind::Special);
        let a = pick_anchor(&s, &eps, &q).unwrap();
        assert_eq!(a, Anchor { season: 0, offset: 4 });
        assert_eq!(map_file(a, &s, None, 1, false), Some((0, 5)));
        assert_eq!(map_file(a, &s, None, 2, false), Some((0, 6)));
        assert_eq!(map_file(a, &s, None, 3, false), None);
        let q = query(&["X"], Some(2018), Some("2018-03-03"), WorkKind::Special);
        assert_eq!(pick_anchor(&s, &eps, &q), None);
        let q = query(&["X"], Some(2015), None, WorkKind::Special);
        assert_eq!(pick_anchor(&s, &eps, &q), None);
    }

    #[test]
    fn titles_fallback() {
        assert_eq!(pick_text("Episódio 3", "Love wins", 3), "Love wins");
        assert_eq!(pick_text("", "", 3), "Episódio 3");
        assert_eq!(pick_text("O fim da aventura", "x", 1), "O fim da aventura");
        assert_eq!(pick_text("Episódio 3", "Episode 3", 3), "Episódio 3");
        assert!(is_generic_title("Episode 12"));
        assert!(is_generic_title(" episódio 7 "));
        assert!(!is_generic_title("O fim da aventura"));
    }

    fn weekly(season: u32, first: u32, dates: &[&str]) -> (u32, Vec<Episode>) {
        let list = dates.iter().enumerate().map(|(i, d)| ep(season, first + i as u32, Some(d))).collect();
        (season, list)
    }

    #[test]
    fn nearest_episode_wins_on_weekly_show() {
        let s = show(&[(1, 24)]);
        let eps = [weekly(1, 11, &["2022-09-18", "2022-09-25", "2022-10-02", "2022-10-09"])];
        // ep12 está a 6 dias, ep13 a 1 dia: o mais próximo vence.
        let q = query(&["X"], Some(2022), Some("2022-10-01"), WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 12 }));
        // Data exata de um episódio, com os vizinhos a 7 dias.
        let q = query(&["X"], Some(2022), Some("2022-10-02"), WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 12 }));
    }

    #[test]
    fn tied_nearest_episodes_no_anchor() {
        let s = show(&[(1, 24)]);
        let eps = [weekly(1, 1, &["2022-09-29", "2022-10-03"])];
        let q = query(&["X"], Some(2022), Some("2022-10-01"), WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), None);
        // -3 e +3 dias, com dois episódios em cada data: continua dúvida.
        let eps = [weekly(1, 1, &["2022-09-28", "2022-09-28", "2022-10-04", "2022-10-04"])];
        assert_eq!(pick_anchor(&s, &eps, &q), None);
        // Mesma data em temporadas diferentes: dúvida.
        let s = show(&[(1, 12), (2, 12)]);
        let eps = [weekly(1, 5, &["2022-10-01"]), weekly(2, 1, &["2022-10-01"])];
        assert_eq!(pick_anchor(&s, &eps, &q), None);
    }

    #[test]
    fn same_day_episodes_anchor_at_lowest() {
        let s = show(&[(1, 12)]);
        let q = query(&["X"], Some(2022), Some("2022-10-01"), WorkKind::Series);
        // Estreia dupla: ep1 e ep2 no mesmo dia.
        let eps = [weekly(1, 1, &["2022-10-01", "2022-10-01", "2022-10-08"])];
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 0 }));
        // Temporada inteira lançada de uma vez (ONA), fora de ordem na lista.
        let mut list: Vec<Episode> = (1..=12).rev().map(|n| ep(1, n, Some("2022-10-03"))).collect();
        list.swap(0, 5);
        let eps = [(1, list)];
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 0 }));
    }

    #[test]
    fn season_numbering_rule_is_disjoint() {
        let special = show(&[(0, 30), (1, 12)]);
        let a = Anchor { season: 0, offset: 10 };
        assert_eq!(map_file(a, &special, None, 1, false), Some((0, 11)));
        assert_eq!(map_file(a, &special, None, 2, false), Some((0, 12)));
        assert_eq!(map_file(a, &special, None, 11, false), Some((0, 21)));
        let s = show(&[(1, 24)]);
        let a = Anchor { season: 1, offset: 5 };
        assert_eq!(map_file(a, &s, None, 6, false), Some((1, 11)));
        assert_eq!(map_file(a, &s, None, 20, true), Some((1, 20)));
    }

    #[test]
    fn exact_title_beats_contains() {
        let hits = [
            hit(30, "Boruto: Naruto Next Generations", "", "2017-04-05"),
            hit(20, "Naruto Shippuden", "", "2007-02-15"),
            hit(10, "Naruto", "", "2002-10-03"),
        ];
        let q = query(&["Naruto"], Some(2023), None, WorkKind::Series);
        assert_eq!(pick_show(&hits, &q), Some(10));
    }

    #[test]
    fn absolute_never_goes_before_anchor() {
        let s = show(&[(1, 61), (2, 16), (3, 14)]);
        let a = Anchor { season: 3, offset: 0 };
        assert_eq!(map_file(a, &s, None, 15, false), None);
    }

    #[test]
    fn explicit_season_only_when_different() {
        let s = show(&[(0, 10), (1, 24), (2, 12)]);
        let a = Anchor { season: 1, offset: 12 };
        assert_eq!(map_file(a, &s, Some(1), 1, false), Some((1, 13)));
        assert_eq!(map_file(a, &s, Some(2), 1, false), Some((2, 1)));
        let a = Anchor { season: 0, offset: 4 };
        assert_eq!(map_file(a, &s, Some(1), 2, false), Some((0, 6)));
        assert_eq!(map_file(a, &s, Some(1), 7, false), None);
    }

    /// Liga uma pasta inteira: o modo de numeração é decidido uma vez para ela.
    fn map_folder(a: Anchor, s: &TvShow, files: impl IntoIterator<Item = u32> + Clone) -> Vec<Option<u32>> {
        let mode = season_numbered(a, files.clone());
        files.into_iter().map(|n| map_file(a, s, None, n, mode).map(|(_, e)| e)).collect()
    }

    #[test]
    fn folder_numbering_decided_once() {
        // Temporada de 25 exibida 12 + 13; a Parte 2 começa no ep. 13.
        let s = show(&[(1, 25)]);
        let a = Anchor { season: 1, offset: 12 };
        let all: Vec<Option<u32>> = (13..=25).map(Some).collect();
        assert_eq!(map_folder(a, &s, 13..=25), all);
        assert_eq!(map_folder(a, &s, 1..=13), all);
        // Pasta mista (1 e 13): relativa.
        assert!(!season_numbered(a, [1, 13]));
        assert_eq!(map_folder(a, &s, [1, 13]), vec![Some(13), Some(25)]);
        // Temporada de 24: os dois jeitos continuam ligando 13–24.
        let s = show(&[(1, 24)]);
        let all: Vec<Option<u32>> = (13..=24).map(Some).collect();
        assert_eq!(map_folder(a, &s, 13..=24), all);
        assert_eq!(map_folder(a, &s, 1..=12), all);
        assert_eq!(map_folder(a, &s, 13..=25).last(), Some(&None));
        // Sem deslocamento ou na temporada 0: nunca numerada pela temporada.
        assert!(!season_numbered(Anchor { season: 1, offset: 0 }, [5, 6]));
        assert!(!season_numbered(Anchor { season: 0, offset: 4 }, [9]));
        assert!(!season_numbered(a, []));
    }

    #[test]
    fn start_without_day_anchors_in_same_month() {
        let s = show(&[(1, 24)]);
        // Último ep. da 1ª parte a 3 dias do "dia 1" assumido; a 2ª parte estreia dia 5.
        let eps = [weekly(1, 12, &["2022-09-28", "2022-10-05", "2022-10-12"])];
        let mut q = query(&["X"], Some(2022), Some("2022-10-01"), WorkKind::Series);
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 11 }));
        q.start_day_known = false;
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 12 }));
        // Nenhum episódio no mês: não liga (mesmo com um vizinho a poucos dias).
        let eps = [weekly(1, 12, &["2022-09-28", "2022-11-02"])];
        assert_eq!(pick_anchor(&s, &eps, &q), None);
        // Mesmo dia em temporadas diferentes: dúvida.
        let s2 = show(&[(1, 12), (2, 12)]);
        let eps = [weekly(1, 5, &["2022-10-20"]), weekly(2, 1, &["2022-10-20"])];
        assert_eq!(pick_anchor(&s2, &eps, &q), None);
        // Episódio no fim do mês ainda vale (mais longe que 7 dias do dia 1).
        let eps = [weekly(1, 1, &["2022-10-23", "2022-10-30"])];
        assert_eq!(pick_anchor(&s, &eps, &q), Some(Anchor { season: 1, offset: 0 }));
        // Especial também.
        let sp = show(&[(0, 3), (1, 12)]);
        let eps = [weekly(0, 1, &["2022-09-29", "2022-10-27"])];
        let mut q = query(&["X"], Some(2022), Some("2022-10-01"), WorkKind::Special);
        q.start_day_known = false;
        assert_eq!(pick_anchor(&sp, &eps, &q), Some(Anchor { season: 0, offset: 1 }));
    }

    #[test]
    fn overview_fallback() {
        assert_eq!(pick_overview("Sinopse", "Synopsis"), "Sinopse");
        assert_eq!(pick_overview("  ", "Synopsis"), "Synopsis");
        assert_eq!(pick_overview("", ""), "");
    }

    fn media(json: &str) -> MediaSummary {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn work_query_skips_movies() {
        let m = media(r#"{"id":1,"format":"MOVIE","title":{"romaji":"Kimi no Na wa."}}"#);
        assert!(work_query(&m, "[2016] Kimi no Na wa").is_none());
    }

    #[test]
    fn work_query_ova_is_special() {
        let m = media(r#"{"id":2,"format":"OVA","title":{"romaji":"Hellsing Ultimate"}}"#);
        assert_eq!(work_query(&m, "Hellsing Ultimate").unwrap().kind, WorkKind::Special);
    }

    #[test]
    fn work_query_tv_uses_full_start_and_folder_title() {
        let m = media(
            r#"{"id":3,"format":"TV","title":{"romaji":"Sousou no Frieren","english":"Frieren: Beyond Journey's End","native":"葬送のフリーレン"},
                "synonyms":["Frieren"],"seasonYear":2023,"startDate":{"year":2023,"month":9,"day":29}}"#,
        );
        let q = work_query(&m, "[2023] Frieren e a Jornada").unwrap();
        assert_eq!(q.kind, WorkKind::Series);
        assert_eq!(q.year, Some(2023));
        assert_eq!(q.start, Some(Ymd { y: 2023, m: 9, d: 29 }));
        assert!(q.start_day_known);
        for t in ["Sousou no Frieren", "Frieren: Beyond Journey's End", "葬送のフリーレン", "Frieren", "Frieren e a Jornada"] {
            assert!(q.titles.iter().any(|q| q == t), "faltou {t}: {:?}", q.titles);
        }
    }

    #[test]
    fn work_query_start_without_day_or_month() {
        let m = media(r#"{"id":4,"format":"TV","title":{"romaji":"X"},"startDate":{"year":2020,"month":4}}"#);
        let q = work_query(&m, "X").unwrap();
        assert_eq!(q.start, Some(Ymd { y: 2020, m: 4, d: 1 }));
        assert!(!q.start_day_known);
        let m = media(r#"{"id":5,"format":"TV","title":{"romaji":"X"},"startDate":{"year":2020}}"#);
        assert_eq!(work_query(&m, "X").unwrap().start, None);
    }
}
