//! Queries GraphQL do AniList.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MediaType {
    Anime,
    Manga,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anime => "ANIME",
            Self::Manga => "MANGA",
        }
    }
}

/// Campos para cards/pôsteres e para o casamento de nomes.
pub const SUMMARY_FRAGMENT: &str = "fragment Summary on Media {
  id type format status
  title { romaji english native userPreferred }
  synonyms
  season seasonYear startDate { year }
  idMal
  coverImage { extraLarge large color }
  bannerImage
  averageScore popularity
  genres
  episodes chapters
}";

/// Página de detalhe: descrição, staff, elenco, estúdio, relações...
pub const DETAIL_QUERY: &str = "query ($id: Int) {
  Media(id: $id) {
    id idMal type format status source countryOfOrigin siteUrl
    title { romaji english native userPreferred }
    synonyms
    description(asHtml: false)
    season seasonYear
    startDate { year month day }
    endDate { year month day }
    episodes duration chapters volumes
    averageScore meanScore popularity favourites
    genres
    tags { name rank isMediaSpoiler }
    coverImage { extraLarge large color }
    bannerImage
    trailer { id site thumbnail }
    studios(isMain: true) { nodes { id name } }
    staff(perPage: 16, sort: [RELEVANCE, ID]) {
      edges { role node { id name { full native } image { large } } }
    }
    characters(perPage: 12, sort: [ROLE, RELEVANCE, ID]) {
      edges {
        role
        node { id name { full native } image { large } }
        voiceActors(language: JAPANESE, sort: [RELEVANCE, ID]) { id name { full native } image { large } }
      }
    }
    relations {
      edges {
        relationType(version: 2)
        node { id type format title { userPreferred romaji native } coverImage { large color } startDate { year } }
      }
    }
    externalLinks { site url }
  }
}";

/// Animes de uma temporada, mais populares primeiro.
pub fn season_query() -> String {
    format!(
        "query ($season: MediaSeason, $year: Int) {{ Page(perPage: 50) {{ media(type: ANIME, season: $season, seasonYear: $year, sort: POPULARITY_DESC, isAdult: false) {{ ...Summary }} }} }} {SUMMARY_FRAGMENT}"
    )
}

/// Lista de ranking da entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ranking {
    /// Mais populares.
    Popular,
    /// Mais bem avaliados.
    TopRated,
    /// Em alta agora (só obras já terminadas).
    Trending,
}

/// Ranking com os extras do banner (sinopse, tags, estúdio).
pub fn ranking_query() -> String {
    format!(
        "query ($type: MediaType, $sort: [MediaSort], $year: Int, $from: FuzzyDateInt, $to: FuzzyDateInt, $status: MediaStatus) {{ Page(perPage: 50) {{ media(type: $type, sort: $sort, seasonYear: $year, startDate_greater: $from, startDate_lesser: $to, status: $status, isAdult: false) {{ ...Summary description(asHtml: false) tags {{ name rank isMediaSpoiler }} studios(isMain: true) {{ nodes {{ name }} }} }} }} }} {SUMMARY_FRAGMENT}"
    )
}

/// Variáveis de [`ranking_query`]; as que não se aplicam ficam de fora.
/// Anime filtra pelo ano da temporada; mangá pela data de início.
pub fn ranking_vars(media: MediaType, ranking: Ranking, year: Option<i32>) -> serde_json::Value {
    let sort = match ranking {
        Ranking::Popular => "POPULARITY_DESC",
        Ranking::TopRated => "SCORE_DESC",
        Ranking::Trending => "TRENDING_DESC",
    };
    let status = (ranking == Ranking::Trending).then_some("FINISHED");
    let (season_year, from, to) = match (media, year) {
        (MediaType::Anime, Some(y)) => (Some(y), None, None),
        (MediaType::Manga, Some(y)) => (None, Some(y * 10_000), Some((y + 1) * 10_000)),
        (_, None) => (None, None, None),
    };
    // Filtro ausente vai omitido: `null` explícito em `startDate_greater` e afins o AniList recusa (400).
    let mut vars = serde_json::json!({ "type": media.as_str(), "sort": sort });
    let obj = vars.as_object_mut().expect("objeto");
    if let Some(y) = season_year {
        obj.insert("year".into(), y.into());
    }
    if let (Some(f), Some(t)) = (from, to) {
        obj.insert("from".into(), f.into());
        obj.insert("to".into(), t.into());
    }
    if let Some(st) = status {
        obj.insert("status".into(), st.into());
    }
    vars
}

/// Ano do calendário (UTC) para um instante Unix.
pub fn year_at(unix_secs: i64) -> i32 {
    civil_year_month(unix_secs.div_euclid(86_400)).0
}

/// Ano atual do calendário (UTC).
pub fn current_year() -> i32 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    year_at(now)
}

/// Temporada de anime (padrão AniList) para um instante Unix.
/// Dezembro já conta como inverno do ano seguinte.
pub fn season_at(unix_secs: i64) -> (&'static str, i32) {
    let (year, month) = civil_year_month(unix_secs.div_euclid(86_400));
    match month {
        12 => ("WINTER", year + 1),
        1 | 2 => ("WINTER", year),
        3..=5 => ("SPRING", year),
        6..=8 => ("SUMMER", year),
        _ => ("FALL", year),
    }
}

/// Dias desde 1970-01-01 → (ano, mês). Algoritmo de Howard Hinnant.
fn civil_year_month(days: i64) -> (i32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month)
}

/// Monta uma query com várias buscas via aliases (`m0`, `m1`, ...), uma requisição só.
pub fn batch_search_query(types: &[MediaType], per_page: u32) -> String {
    let defs: Vec<String> = (0..types.len()).map(|i| format!("$s{i}: String")).collect();
    let body: Vec<String> = types
        .iter()
        .enumerate()
        .map(|(i, t)| {
            format!(
                "m{i}: Page(perPage: {per_page}) {{ media(search: $s{i}, type: {}, sort: SEARCH_MATCH) {{ ...Summary }} }}",
                t.as_str()
            )
        })
        .collect();
    format!(
        "query ({}) {{ {} }} {SUMMARY_FRAGMENT}",
        defs.join(", "),
        body.join(" ")
    )
}

/// Busca várias mídias pelo id da MAL em uma requisição só (aliases `m0`, `m1`, ...).
pub fn idmal_batch_query(n: usize) -> String {
    let defs: Vec<String> = (0..n).map(|i| format!("$i{i}: Int")).collect();
    let body: Vec<String> = (0..n).map(|i| format!("m{i}: Media(idMal: $i{i}, type: ANIME) {{ ...Summary }}")).collect();
    format!("query ({}) {{ {} }} {SUMMARY_FRAGMENT}", defs.join(", "), body.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn season_from_date() {
        // 2026-11-01
        assert_eq!(season_at(1_793_491_200), ("FALL", 2026));
        // 2026-12-15 → inverno de 2027
        assert_eq!(season_at(1_797_292_800), ("WINTER", 2027));
        // 2024-02-29
        assert_eq!(season_at(1_709_164_800), ("WINTER", 2024));
        // 2025-06-01
        assert_eq!(season_at(1_748_736_000), ("SUMMER", 2025));
    }

    #[test]
    fn calendar_year() {
        assert_eq!(year_at(1_797_292_800), 2026); // 2026-12-15
        assert_eq!(year_at(1_798_761_599), 2026); // 2026-12-31 23:59:59
        assert_eq!(year_at(1_798_761_600), 2027); // 2027-01-01
    }

    #[test]
    fn ranking_vars_anime_year() {
        let v = ranking_vars(MediaType::Anime, Ranking::Popular, Some(2026));
        assert_eq!(v["type"], "ANIME");
        assert_eq!(v["sort"], "POPULARITY_DESC");
        assert_eq!(v["year"], 2026);
        assert!(v.get("from").is_none() && v.get("to").is_none() && v.get("status").is_none());
    }

    #[test]
    fn ranking_vars_manga_year() {
        let v = ranking_vars(MediaType::Manga, Ranking::TopRated, Some(2026));
        assert_eq!(v["type"], "MANGA");
        assert_eq!(v["sort"], "SCORE_DESC");
        assert_eq!(v["from"], 20260000);
        assert_eq!(v["to"], 20270000);
        assert!(v.get("year").is_none());
    }

    #[test]
    fn ranking_vars_trending_finished() {
        let v = ranking_vars(MediaType::Anime, Ranking::Trending, None);
        assert_eq!(v["sort"], "TRENDING_DESC");
        assert_eq!(v["status"], "FINISHED");
        assert!(v.get("year").is_none() && v.get("from").is_none());
    }

    #[test]
    fn ranking_query_asks_extras() {
        let q = ranking_query();
        assert!(q.contains("description(asHtml: false)"));
        assert!(q.contains("isMediaSpoiler"));
        assert!(q.contains("studios(isMain: true)"));
        assert!(q.contains("fragment Summary on Media"));
    }

    #[test]
    fn builds_batch_query() {
        let q = batch_search_query(&[MediaType::Anime, MediaType::Manga], 6);
        assert!(q.starts_with("query ($s0: String, $s1: String)"));
        assert!(q.contains("m0: Page(perPage: 6) { media(search: $s0, type: ANIME"));
        assert!(q.contains("m1: Page(perPage: 6) { media(search: $s1, type: MANGA"));
        assert!(q.contains("fragment Summary on Media"));
    }
}
