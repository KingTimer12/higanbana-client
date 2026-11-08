//! Cliente da Jikan (API não oficial da MyAnimeList, https://api.jikan.moe/v4), só leitura.
//!
//! Serve de complemento ao AniList: obras que existem só na MAL ganham id `-mal_id`
//! e os itens são convertidos para o formato do AniList (`Summary`/detalhe).

use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode};
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

use super::error::{AniListError, AniListResult};

const ENDPOINT: &str = "https://api.jikan.moe/v4";
const MIN_INTERVAL: Duration = Duration::from_secs(1);
const RETRY_DELAY: Duration = Duration::from_secs(2);
const MAX_ATTEMPTS: usize = 3;

/// Cliente da Jikan: no máximo 1 requisição por segundo; em `429` espera 2 s e tenta de novo.
pub struct JikanClient {
    http: Client,
    last_request: Mutex<Option<Instant>>,
}

impl JikanClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("falha ao criar cliente HTTP"),
            last_request: Mutex::new(None),
        }
    }

    /// Busca por texto; itens já no formato `Summary` do AniList.
    pub async fn search(&self, q: &str) -> AniListResult<Vec<Value>> {
        let body = self.get("/anime", &[("q", q), ("limit", "6"), ("sfw", "false")]).await?;
        let items = body.get("data").and_then(Value::as_array).ok_or_else(|| AniListError::Decode("resposta da Jikan sem `data`".into()))?;
        Ok(items.iter().map(to_summary_shape).collect())
    }

    /// Detalhe de um anime pelo id da MAL, no formato de detalhe do AniList.
    pub async fn anime(&self, mal_id: i64) -> AniListResult<Value> {
        let body = self.get(&format!("/anime/{mal_id}"), &[]).await?;
        let item = body.get("data").filter(|d| d.is_object()).ok_or(AniListError::NotFound)?;
        Ok(to_detail_shape(item))
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> AniListResult<Value> {
        for attempt in 0..MAX_ATTEMPTS {
            self.space_out().await;
            let resp = self.http.get(format!("{ENDPOINT}{path}")).query(query).send().await?;
            let status = resp.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                if attempt + 1 < MAX_ATTEMPTS {
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                continue;
            }
            if status == StatusCode::NOT_FOUND {
                return Err(AniListError::NotFound);
            }
            let text = resp.text().await?;
            return serde_json::from_str(&text).map_err(|e| AniListError::Decode(format!("{e} (HTTP {status})")));
        }
        Err(AniListError::RateLimited)
    }

    /// Garante ≥ 1 s entre o início de duas requisições, mesmo com chamadas concorrentes.
    async fn space_out(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(prev) = *last {
            let ready = prev + MIN_INTERVAL;
            if ready > Instant::now() {
                tokio::time::sleep_until(ready.into()).await;
            }
        }
        *last = Some(Instant::now());
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn names(item: &Value, key: &str) -> Vec<String> {
    item.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|g| str_of(g, "name")).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn format_of(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "TV" => "TV",
        "Movie" => "MOVIE",
        "OVA" => "OVA",
        "ONA" => "ONA",
        "Special" | "TV Special" => "SPECIAL",
        "Music" => "MUSIC",
        _ => return None,
    })
}

fn cover_of(item: &Value) -> Option<String> {
    let images = item.get("images")?;
    ["webp", "jpg"].iter().find_map(|fmt| {
        let img = images.get(fmt)?;
        str_of(img, "large_image_url").or_else(|| str_of(img, "image_url")).map(str::to_owned)
    })
}

/// Item da Jikan → formato `Summary` do AniList, com `id = -mal_id`.
pub fn to_summary_shape(item: &Value) -> Value {
    let mal_id = item.get("mal_id").and_then(Value::as_i64).unwrap_or(0);
    let from = item.pointer("/aired/prop/from");
    let year = item.get("year").and_then(Value::as_i64).or_else(|| from.and_then(|f| f.get("year")).and_then(Value::as_i64));
    let romaji = str_of(item, "title");
    let cover = cover_of(item).map(|url| json!({ "extraLarge": url, "large": url }));
    let mut genres = names(item, "genres");
    genres.extend(names(item, "themes"));

    json!({
        "id": -mal_id,
        "idMal": mal_id,
        "type": "ANIME",
        "format": str_of(item, "type").and_then(format_of),
        "title": {
            "romaji": romaji,
            "english": str_of(item, "title_english"),
            "native": str_of(item, "title_japanese"),
            "userPreferred": romaji,
        },
        "synonyms": item.get("title_synonyms").cloned().unwrap_or_else(|| json!([])),
        "season": str_of(item, "season").map(str::to_uppercase),
        "seasonYear": year,
        "startDate": { "year": year },
        "coverImage": cover,
        "averageScore": item.get("score").and_then(Value::as_f64).map(|s| (s * 10.0).round() as i64),
        "genres": genres,
        "episodes": item.get("episodes").and_then(Value::as_i64),
    })
}

/// Item da Jikan → formato de detalhe do AniList (sem elenco, equipe nem relações).
pub fn to_detail_shape(item: &Value) -> Value {
    let mut shape = to_summary_shape(item);
    let studios: Vec<Value> = item
        .get("studios")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| Some(json!({ "id": s.get("mal_id")?.as_i64()?, "name": str_of(s, "name")? }))).collect())
        .unwrap_or_default();
    let obj: &mut Map<String, Value> = shape.as_object_mut().expect("summary é objeto");
    obj.insert("description".into(), json!(str_of(item, "synopsis")));
    obj.insert("siteUrl".into(), json!(str_of(item, "url")));
    obj.insert("studios".into(), json!({ "nodes": studios }));
    for key in ["characters", "staff", "relations"] {
        obj.insert(key.into(), json!({ "edges": [] }));
    }
    shape
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::anilist::queries::idmal_batch_query;
    use crate::anilist::types::MediaSummary;

    fn fixture() -> Value {
        json!({
            "mal_id": 1531,
            "url": "https://myanimelist.net/anime/1531/Fushigiboshi_no☆Futago_Hime",
            "images": {
                "jpg": {
                    "image_url": "https://cdn.myanimelist.net/images/anime/9/20213.jpg",
                    "large_image_url": "https://cdn.myanimelist.net/images/anime/9/20213l.jpg"
                },
                "webp": {
                    "image_url": "https://cdn.myanimelist.net/images/anime/9/20213.webp",
                    "large_image_url": "https://cdn.myanimelist.net/images/anime/9/20213l.webp"
                }
            },
            "title": "Fushigiboshi no☆Futago Hime",
            "title_english": "Twin Princesses of the Wonder Planet",
            "title_japanese": "ふしぎ星の☆ふたご姫",
            "title_synonyms": ["Twinkle Princess"],
            "type": "TV",
            "episodes": 51,
            "aired": { "from": "2005-04-02T00:00:00+00:00", "prop": { "from": { "day": 2, "month": 4, "year": 2005 } } },
            "score": 7.2,
            "season": "spring",
            "year": 2005,
            "synopsis": "Fine and Rein are twin princesses.",
            "genres": [{ "mal_id": 10, "name": "Fantasy" }],
            "themes": [],
            "studios": [{ "mal_id": 18, "name": "Bones" }]
        })
    }

    #[test]
    fn jikan_item_to_anilist_shape() {
        let s = to_summary_shape(&fixture());
        let m: MediaSummary = serde_json::from_value(s).unwrap();
        assert_eq!(m.id, -1531);
        assert_eq!(m.id_mal, Some(1531));
        assert!(m.is_mal_only());
        assert_eq!(m.format.as_deref(), Some("TV"));
        assert_eq!(m.season.as_deref(), Some("SPRING"));
        assert_eq!(m.year(), Some(2005));
        assert_eq!(m.average_score, Some(72));
        assert!(m.genres.contains(&"Fantasy".to_string()));
        assert!(m.cover().unwrap().starts_with("https://"));
    }

    #[test]
    fn detail_shape_has_empty_people() {
        let d: crate::anilist::types::MediaDetail = serde_json::from_value(to_detail_shape(&fixture())).unwrap();
        assert!(d.characters.edges.is_empty() && d.staff.edges.is_empty() && d.relations.edges.is_empty());
        assert_eq!(d.studios.nodes[0].name, "Bones");
        assert_eq!(d.description.as_deref(), Some("Fine and Rein are twin princesses."));
        assert!(d.site_url.unwrap().contains("myanimelist.net"));
    }

    #[test]
    fn maps_formats() {
        for (j, a) in [("Movie", "MOVIE"), ("OVA", "OVA"), ("ONA", "ONA"), ("Special", "SPECIAL"), ("TV Special", "SPECIAL"), ("Music", "MUSIC")] {
            let mut f = fixture();
            f["type"] = json!(j);
            assert_eq!(to_summary_shape(&f)["format"], a);
        }
    }

    #[test]
    fn builds_idmal_query() {
        let q = idmal_batch_query(2);
        assert!(q.starts_with("query ($i0: Int, $i1: Int)"));
        assert!(q.contains("m1: Media(idMal: $i1, type: ANIME)"));
    }
}
