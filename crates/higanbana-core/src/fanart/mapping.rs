//! Mapa AniList/MAL → alvo no fanart.tv, montado a partir da lista Fribb
//! (`anime-list-full.json`).
//!
//! Formato real da lista (array de objetos): `type` ("TV", "MOVIE", ...),
//! `anilist_id` e `mal_id` (inteiros ou ausentes), `tvdb_id` (inteiro),
//! `themoviedb_id` (objeto `{"tv": 123}` ou `{"movie": [128]}`) e `season`
//! (objeto `{"tvdb": 2, "tmdb": 2}`). Numeração absoluta aparece como
//! `season` ausente/nula ou sem `tvdb`; temporada 0 são especiais.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{FanartError, FanartResult};

/// Onde procurar a arte no fanart.tv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// `series_poster`: aceita `tvposter` quando não há `seasonposter` (só temporada 1).
    Season { tvdb: u32, season: u32, series_poster: bool },
    Movie { tmdb: u32 },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnimeMap {
    by_anilist: HashMap<i64, Target>,
    by_mal: HashMap<i64, Target>,
}

/// Aceita número ou string numérica; qualquer outra coisa vira `None`.
fn lenient_id<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawEntry {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(deserialize_with = "lenient_id")]
    anilist_id: Option<i64>,
    #[serde(deserialize_with = "lenient_id")]
    mal_id: Option<i64>,
    #[serde(deserialize_with = "lenient_id")]
    tvdb_id: Option<i64>,
    themoviedb_id: Option<Value>,
    season: Option<Value>,
}

impl RawEntry {
    /// ID do filme no TMDB (`{"movie": [128]}` ou `{"movie": 128}`).
    fn tmdb_movie(&self) -> Option<u32> {
        let v = self.themoviedb_id.as_ref()?.get("movie")?;
        let n = match v {
            Value::Array(a) => a.first()?.as_i64()?,
            other => other.as_i64()?,
        };
        u32::try_from(n).ok().filter(|n| *n > 0)
    }

    /// `Some(n)` com a temporada do TVDB; `None` quando ausente (numeração absoluta).
    fn tvdb_season(&self) -> Option<u32> {
        let n = self.season.as_ref()?.get("tvdb")?.as_i64()?;
        u32::try_from(n).ok()
    }

    fn has_id(&self) -> bool {
        self.anilist_id.is_some() || self.mal_id.is_some()
    }
}

impl AnimeMap {
    pub fn from_fribb(json: &[u8]) -> FanartResult<Self> {
        let raw: Vec<RawEntry> =
            serde_json::from_slice(json).map_err(|e| FanartError::Decode(e.to_string()))?;
        let mut map = Self::default();

        // Entradas de série (não-filme) com tvdb e algum id, para as regras 4 e 5.
        let mut tv: Vec<(u32, Option<u32>, &RawEntry)> = Vec::new();
        let mut per_tvdb: HashMap<u32, usize> = HashMap::new();
        for e in raw.iter().filter(|e| e.has_id()) {
            if e.kind.as_deref() == Some("MOVIE") {
                if let Some(tmdb) = e.tmdb_movie() {
                    map.insert(e, Target::Movie { tmdb });
                    continue;
                }
            }
            let Some(tvdb) = e
                .tvdb_id
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n > 0)
            else {
                continue;
            };
            *per_tvdb.entry(tvdb).or_default() += 1;
            tv.push((tvdb, e.tvdb_season(), e));
        }

        // Grupos (tvdb, temporada): só a "primeira parte" recebe o alvo.
        // Menor anilist_id; sem anilist_id, o menor mal_id desempata.
        let rank = |e: &RawEntry| (e.anilist_id.unwrap_or(i64::MAX), e.mal_id.unwrap_or(i64::MAX));
        let mut groups: HashMap<(u32, u32), &RawEntry> = HashMap::new();
        for (tvdb, season, e) in tv {
            let season = match season {
                Some(0) => continue,
                Some(s) => s,
                None if per_tvdb[&tvdb] == 1 => 1,
                None => continue,
            };
            groups
                .entry((tvdb, season))
                .and_modify(|cur| {
                    if rank(e) < rank(cur) {
                        *cur = e;
                    }
                })
                .or_insert(e);
        }
        for ((tvdb, season), e) in groups {
            map.insert(
                e,
                Target::Season { tvdb, season, series_poster: season == 1 },
            );
        }
        Ok(map)
    }

    fn insert(&mut self, e: &RawEntry, target: Target) {
        if let Some(id) = e.anilist_id.filter(|id| *id > 0) {
            self.by_anilist.insert(id, target);
        }
        if let Some(id) = e.mal_id.filter(|id| *id > 0) {
            self.by_mal.insert(id, target);
        }
    }

    /// `anilist_id > 0` → AniList; senão (ou não achou) → MAL com `mal_id`
    /// (ou `-anilist_id` quando negativo).
    pub fn target(&self, anilist_id: i64, mal_id: Option<i64>) -> Option<Target> {
        if anilist_id > 0 {
            if let Some(t) = self.by_anilist.get(&anilist_id) {
                return Some(*t);
            }
        }
        let mal = mal_id.or_else(|| (anilist_id < 0).then_some(-anilist_id))?;
        self.by_mal.get(&mal).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(json: &str) -> AnimeMap {
        AnimeMap::from_fribb(json.as_bytes()).unwrap()
    }

    #[test]
    fn movie_maps_to_tmdb() {
        let m = map(r#"[{"type":"MOVIE","anilist_id":1,"themoviedb_id":{"movie":[123]}}]"#);
        assert_eq!(m.target(1, None), Some(Target::Movie { tmdb: 123 }));
    }

    #[test]
    fn season_one_accepts_series_poster() {
        let m = map(r#"[{"type":"TV","anilist_id":1,"tvdb_id":5,"season":{"tvdb":1}}]"#);
        assert_eq!(
            m.target(1, None),
            Some(Target::Season { tvdb: 5, season: 1, series_poster: true })
        );
    }

    #[test]
    fn later_season_no_series_poster() {
        let m = map(r#"[{"type":"TV","anilist_id":1,"tvdb_id":5,"season":{"tvdb":2}}]"#);
        assert_eq!(
            m.target(1, None),
            Some(Target::Season { tvdb: 5, season: 2, series_poster: false })
        );
    }

    #[test]
    fn season_zero_is_skipped() {
        let m = map(r#"[{"type":"TV","anilist_id":1,"tvdb_id":5,"season":{"tvdb":0}}]"#);
        assert_eq!(m.target(1, None), None);
    }

    #[test]
    fn split_season_only_first_part() {
        let m = map(
            r#"[{"anilist_id":200,"mal_id":20,"tvdb_id":5,"season":{"tvdb":2}},
                {"anilist_id":100,"mal_id":10,"tvdb_id":5,"season":{"tvdb":2}}]"#,
        );
        assert!(m.target(100, None).is_some());
        assert_eq!(m.target(200, None), None);
        assert_eq!(m.target(0, Some(20)), None);
        assert!(m.target(0, Some(10)).is_some());
    }

    #[test]
    fn split_season_without_anilist_uses_smallest_mal() {
        let m = map(
            r#"[{"mal_id":20,"tvdb_id":5,"season":{"tvdb":2}},
                {"mal_id":10,"tvdb_id":5,"season":{"tvdb":2}}]"#,
        );
        assert!(m.target(0, Some(10)).is_some());
        assert_eq!(m.target(0, Some(20)), None);
    }

    #[test]
    fn absolute_single_entry_is_season_one() {
        let t = Some(Target::Season { tvdb: 5, season: 1, series_poster: true });
        let m = map(r#"[{"type":"TV","anilist_id":1,"tvdb_id":5}]"#);
        assert_eq!(m.target(1, None), t);
        let m = map(r#"[{"anilist_id":1,"tvdb_id":5,"season":{"tmdb":1}}]"#);
        assert_eq!(m.target(1, None), t);
    }

    #[test]
    fn absolute_with_siblings_is_skipped() {
        let m = map(
            r#"[{"anilist_id":1,"tvdb_id":5},
                {"anilist_id":2,"tvdb_id":5,"season":{"tvdb":2}}]"#,
        );
        assert_eq!(m.target(1, None), None);
        assert!(m.target(2, None).is_some());
    }

    #[test]
    fn mal_only_by_negative_id() {
        let m = map(r#"[{"mal_id":77,"tvdb_id":5,"season":{"tvdb":1}}]"#);
        assert!(m.target(-77, None).is_some());
        assert!(m.target(0, Some(77)).is_some());
        assert_eq!(m.target(-78, None), None);
    }

    #[test]
    fn junk_entries_ignored() {
        let m = map(
            r#"[{}, {"type":"TV"}, {"anilist_id":null,"mal_id":null,"tvdb_id":5},
                {"anilist_id":"abc","tvdb_id":"x"}, {"type":"MOVIE","anilist_id":3},
                {"anilist_id":9,"tvdb_id":6,"season":{"tvdb":1}},
                {"anilist_id":9,"tvdb_id":6,"season":{"tvdb":1}}]"#,
        );
        assert!(m.target(9, None).is_some());
        assert_eq!(m.target(3, None), None);
        assert!(matches!(
            AnimeMap::from_fribb(b"nao e json"),
            Err(FanartError::Decode(_))
        ));
    }

    #[test]
    fn roundtrips_json() {
        let m = map(r#"[{"anilist_id":1,"mal_id":2,"tvdb_id":5,"season":{"tvdb":1}}]"#);
        let back: AnimeMap = serde_json::from_slice(&serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(back.target(1, None), m.target(1, None));
        assert_eq!(back.target(0, Some(2)), m.target(0, Some(2)));
    }
}
