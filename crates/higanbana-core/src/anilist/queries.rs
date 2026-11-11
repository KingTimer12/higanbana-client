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
    fn builds_batch_query() {
        let q = batch_search_query(&[MediaType::Anime, MediaType::Manga], 6);
        assert!(q.starts_with("query ($s0: String, $s1: String)"));
        assert!(q.contains("m0: Page(perPage: 6) { media(search: $s0, type: ANIME"));
        assert!(q.contains("m1: Page(perPage: 6) { media(search: $s1, type: MANGA"));
        assert!(q.contains("fragment Summary on Media"));
    }
}
