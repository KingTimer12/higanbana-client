//! Casamento puro obra/arquivo → episódio do TMDB. Na dúvida, não liga nada.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::types::{Episode, TvHit, TvShow};

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

/// Série do TMDB da obra: o título tem que casar; depois o ano de estreia igual ao
/// da obra; senão a estreia mais recente antes dele (parte posterior → série-mãe).
/// Sem ano, o primeiro que casa pelo título.
pub fn pick_show(hits: &[TvHit], q: &WorkQuery) -> Option<u64> {
    let titles: Vec<String> = q.titles.iter().map(|t| normalize(t)).filter(|t| !t.is_empty()).collect();
    let matches = |h: &&TvHit| {
        [&h.name, &h.original_name]
            .into_iter()
            .map(|n| normalize(n))
            .any(|n| !n.is_empty() && titles.iter().any(|t| n.contains(t.as_str())))
    };
    let found: Vec<&TvHit> = hits.iter().filter(matches).collect();
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

/// Primeiro episódio (em ordem de temporada e número) que estreou a até
/// `DATE_TOLERANCE` dias de `start`.
fn anchor_near<'a>(seasons: impl Iterator<Item = &'a (u32, Vec<Episode>)>, start: Ymd) -> Option<Anchor> {
    let mut eps: Vec<(u32, &Episode)> = seasons.flat_map(|(s, list)| list.iter().map(move |e| (*s, e))).collect();
    eps.sort_by_key(|(s, e)| (*s, e.episode_number));
    eps.into_iter().find_map(|(season, e)| {
        let date = parse_date(e.air_date.as_deref())?;
        if (date.days() - start.days()).abs() > DATE_TOLERANCE {
            return None;
        }
        Some(Anchor { season, offset: e.episode_number.checked_sub(1)? })
    })
}

/// Onde a obra começa dentro da série. `Series`: temporadas regulares, pelo
/// episódio que estreou junto com a obra; sem data, só se houver uma única
/// temporada regular. `Special`: só a temporada 0, e só com data.
pub fn pick_anchor(show: &TvShow, seasons: &[(u32, Vec<Episode>)], q: &WorkQuery) -> Option<Anchor> {
    match q.kind {
        WorkKind::Series => {
            if let Some(start) = q.start {
                return anchor_near(seasons.iter().filter(|(s, _)| *s > 0), start);
            }
            let regular: Vec<u32> = show.seasons.iter().map(|s| s.season_number).filter(|&s| s > 0).collect();
            match regular[..] {
                [season] => Some(Anchor { season, offset: 0 }),
                _ => None,
            }
        }
        WorkKind::Special => anchor_near(seasons.iter().filter(|(s, _)| *s == 0), q.start?),
    }
}

/// Arquivo de número `number` → `(temporada, episódio)` no TMDB. Regras, em ordem:
/// 1. temporada explícita no nome (`S02E03`) que existe no TMDB vence; se o número
///    não cabe nela, não liga (dúvida);
/// 2. âncora com deslocamento e `offset < number <= total` da temporada: arquivos
///    numerados pela própria temporada (ex. 13–24 numa obra que começa no 13) →
///    `(âncora, number)`;
/// 3. `offset + number <= total` → `(âncora, offset + number)`;
/// 4. só com `offset == 0` numa temporada regular: numeração absoluta, somando os
///    totais das temporadas regulares a partir da 1 (caso One Piece);
/// 5. nada disso → `None`.
pub fn map_file(anchor: Anchor, show: &TvShow, file_season: Option<u32>, number: u32) -> Option<(u32, u32)> {
    if number == 0 {
        return None;
    }
    let count = |season: u32| show.seasons.iter().find(|s| s.season_number == season).map(|s| s.episode_count);
    if let Some(season) = file_season
        && let Some(total) = count(season) {
            return (number <= total).then_some((season, number));
        }
    let total = count(anchor.season)?;
    if anchor.offset > 0 && number > anchor.offset && number <= total {
        return Some((anchor.season, number));
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
            return Some((season, rest));
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
        WorkQuery { titles: titles.iter().map(|t| (*t).into()).collect(), year, start: start.map(ymd), kind }
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
        assert_eq!(map_file(a, &s, None, 1), Some((1, 13)));
        assert_eq!(map_file(a, &s, None, 13), Some((1, 13)));
        assert_eq!(map_file(a, &s, None, 25), None);
        assert_eq!(map_file(a, &s, None, 0), None);
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
        assert_eq!(map_file(a, &s, None, 5), Some((1, 5)));
        assert_eq!(map_file(a, &s, None, 70), Some((2, 9)));
        assert_eq!(map_file(a, &s, None, 91), Some((3, 14)));
        assert_eq!(map_file(a, &s, None, 92), None);
    }

    #[test]
    fn explicit_season_wins() {
        let s = show(&[(1, 61), (2, 16)]);
        let a = Anchor { season: 1, offset: 0 };
        assert_eq!(map_file(a, &s, Some(2), 3), Some((2, 3)));
        assert_eq!(map_file(a, &s, Some(9), 3), Some((1, 3)));
        // Temporada explícita existe mas o número não cabe: dúvida, não liga.
        assert_eq!(map_file(a, &s, Some(2), 20), None);
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
        assert_eq!(map_file(a, &s, None, 1), Some((0, 5)));
        assert_eq!(map_file(a, &s, None, 2), Some((0, 6)));
        assert_eq!(map_file(a, &s, None, 3), None);
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
}
