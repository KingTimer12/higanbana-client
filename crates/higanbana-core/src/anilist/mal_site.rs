//! Busca da MyAnimeList pelo próprio site (`myanimelist.net/search/prefix.json`), só leitura.
//!
//! É o endpoint do autocompletar do site: público, sem chave, e alcançável onde a Jikan
//! (api.jikan.moe) não é. Os itens viram o formato `Summary` do AniList com `id = -mal_id`.

use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{
    error::{AniListError, AniListResult},
    jikan::format_of,
};

const ENDPOINT: &str = "https://myanimelist.net/search/prefix.json";
/// Educado com o site: no máximo ~1 requisição por segundo.
const MIN_INTERVAL: Duration = Duration::from_millis(1100);
const RETRY_DELAY: Duration = Duration::from_secs(2);
const MAX_ATTEMPTS: usize = 3;
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Cliente da busca do site da MAL: ≥ 1,1 s entre chamadas; em `429` espera 2 s e tenta de novo.
pub struct MalSiteClient {
    http: Client,
    last_request: Mutex<Option<Instant>>,
}

impl Default for MalSiteClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MalSiteClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(TOTAL_TIMEOUT)
                .build()
                .expect("falha ao criar cliente HTTP"),
            last_request: Mutex::new(None),
        }
    }

    /// Busca por texto; itens já no formato `Summary` do AniList.
    pub async fn search(&self, q: &str) -> AniListResult<Vec<Value>> {
        let body = self.get(&[("type", "anime"), ("keyword", q), ("v", "1")]).await?;
        Ok(prefix_items(&body).iter().map(prefix_item_to_summary).collect())
    }

    async fn get(&self, query: &[(&str, &str)]) -> AniListResult<Value> {
        for attempt in 0..MAX_ATTEMPTS {
            self.space_out().await;
            let resp = self.http.get(ENDPOINT).query(query).send().await?;
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

    /// Garante o intervalo entre o início de duas requisições, mesmo com chamadas concorrentes.
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

/// Itens da categoria `anime` da resposta (vazio se não houver).
pub fn prefix_items(body: &Value) -> Vec<Value> {
    body.get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|c| c.get("type").and_then(Value::as_str) == Some("anime"))
        .filter_map(|c| c.get("items").and_then(Value::as_array))
        .flatten()
        .cloned()
        .collect()
}

/// Miniatura do site → imagem grande: sem o segmento `/r/<largura>x<altura>` nem a query.
pub fn large_image(url: &str) -> String {
    let url = url.split('?').next().unwrap_or(url);
    if let Some(start) = url.find("/r/") {
        let rest = &url[start + 3..];
        if let Some(end) = rest.find('/') {
            let size = &rest[..end];
            let is_size = size.split_once('x').is_some_and(|(w, h)| {
                !w.is_empty() && !h.is_empty() && w.bytes().all(|b| b.is_ascii_digit()) && h.bytes().all(|b| b.is_ascii_digit())
            });
            if is_size {
                return format!("{}{}", &url[..start], &rest[end..]);
            }
        }
    }
    url.to_owned()
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Item do `prefix.json` → formato `Summary` do AniList, com `id = -mal_id`.
pub fn prefix_item_to_summary(item: &Value) -> Value {
    let mal_id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
    let payload = item.get("payload").cloned().unwrap_or(Value::Null);
    let year = payload.get("start_year").and_then(Value::as_i64);
    let name = str_of(item, "name");
    let cover = str_of(item, "image_url").map(large_image).map(|url| json!({ "extraLarge": url, "large": url }));
    // O site manda a nota como texto ("6.88") ou "N/A".
    let score = payload.get("score").and_then(|s| s.as_f64().or_else(|| s.as_str().and_then(|t| t.trim().parse::<f64>().ok())));

    json!({
        "id": -mal_id,
        "idMal": mal_id,
        "type": "ANIME",
        "format": str_of(&payload, "media_type").and_then(format_of),
        "title": {
            "romaji": name,
            "english": null,
            "native": null,
            "userPreferred": name,
        },
        "synonyms": [],
        "seasonYear": year,
        "startDate": { "year": year },
        "coverImage": cover,
        "averageScore": score.map(|s| (s * 10.0).round() as i64),
        "genres": [],
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::anilist::{
        matcher::{parse_folder_name, pick_best},
        types::MediaSummary,
    };

    fn body() -> Value {
        json!({"categories":[{"type":"anime","items":[
            {"id":21407,"type":"anime","name":"Happiness Charge Precure!",
             "url":"https://myanimelist.net/anime/21407/Happiness_Charge_Precure",
             "image_url":"https://cdn.myanimelist.net/r/116x180/images/anime/1973/94568.jpg?s=df961a8eb685fe1cda5aea2663679b8c",
             "thumbnail_url":"https://cdn.myanimelist.net/r/116x76/images/anime/1973/94568.jpg?s=73c3509aa020422256e6ec51c77ce8d9",
             "payload":{"media_type":"TV","start_year":2014,"aired":"Feb 2, 2014 to Jan 25, 2015","score":"6.88","status":"Finished Airing"},
             "es_score":97.672516},
            {"id":38578,"type":"anime","name":"Star☆Twinkle Precure",
             "url":"https://myanimelist.net/anime/38578/Star☆Twinkle_Precure",
             "image_url":"https://cdn.myanimelist.net/r/116x180/images/anime/1585/97907.jpg?s=abc",
             "thumbnail_url":"https://cdn.myanimelist.net/r/116x76/images/anime/1585/97907.jpg?s=def",
             "payload":{"media_type":"Movie","start_year":2019,"aired":"Feb 3, 2019 to Jan 26, 2020","score":"N/A","status":"Finished Airing"},
             "es_score":67.4}
        ]}]})
    }

    #[test]
    fn strips_resize_and_query() {
        assert_eq!(
            large_image("https://cdn.myanimelist.net/r/116x180/images/anime/1585/97907.jpg?s=abc"),
            "https://cdn.myanimelist.net/images/anime/1585/97907.jpg"
        );
        // Já grande: fica igual.
        assert_eq!(large_image("https://cdn.myanimelist.net/images/anime/9/20213.jpg"), "https://cdn.myanimelist.net/images/anime/9/20213.jpg");
    }

    #[test]
    fn reads_anime_items() {
        assert_eq!(prefix_items(&body()).len(), 2);
        assert!(prefix_items(&json!({"categories":[{"type":"anime","items":[]}]})).is_empty());
        assert!(prefix_items(&json!({"errors":[]})).is_empty());
        assert!(prefix_items(&json!({"categories":[{"type":"manga","items":[{"id":1}]}]})).is_empty());
    }

    #[test]
    fn prefix_item_to_anilist_shape() {
        let items = prefix_items(&body());
        let s = prefix_item_to_summary(&items[0]);
        assert_eq!(s["type"], "ANIME");
        let m: MediaSummary = serde_json::from_value(s).unwrap();
        assert_eq!(m.id, -21407);
        assert_eq!(m.id_mal, Some(21407));
        assert!(m.is_mal_only());
        assert_eq!(m.title(), "Happiness Charge Precure!");
        assert_eq!(m.format.as_deref(), Some("TV"));
        assert_eq!(m.year(), Some(2014));
        assert_eq!(m.start_date.and_then(|d| d.year), Some(2014));
        assert_eq!(m.average_score, Some(69));
        assert_eq!(m.cover(), Some("https://cdn.myanimelist.net/images/anime/1973/94568.jpg"));
        assert_eq!(m.cover_small(), Some("https://cdn.myanimelist.net/images/anime/1973/94568.jpg"));
    }

    #[test]
    fn non_numeric_score_and_formats() {
        let items = prefix_items(&body());
        let s = prefix_item_to_summary(&items[1]);
        assert!(s["averageScore"].is_null());
        assert_eq!(s["format"], "MOVIE");
        for (j, a) in [("OVA", "OVA"), ("ONA", "ONA"), ("Special", "SPECIAL"), ("TV Special", "SPECIAL"), ("Music", "MUSIC")] {
            let mut it = items[0].clone();
            it["payload"]["media_type"] = json!(j);
            assert_eq!(prefix_item_to_summary(&it)["format"], a);
        }
    }

    #[test]
    fn matcher_picks_site_result() {
        let candidates: Vec<Value> = prefix_items(&body()).iter().map(prefix_item_to_summary).collect();
        let wanted = parse_folder_name("[2014.0] Happiness Charge Precure! [TV]");
        assert_eq!(candidates[pick_best(&candidates, &wanted).unwrap()]["idMal"], 21407);
    }

    /// Rede de verdade (sem login): `cargo test -p higanbana-core live_mal_site -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn live_mal_site_then_anilist_by_idmal() {
        let found = MalSiteClient::new().search("Happiness Charge Precure!").await.unwrap();
        let wanted = parse_folder_name("[2014.0] Happiness Charge Precure! [TV]");
        let best = &found[pick_best(&found, &wanted).unwrap()];
        println!("MAL: idMal={} título={} capa={}", best["idMal"], best["title"]["romaji"], best["coverImage"]["large"]);
        assert_eq!(best["idMal"], 21407);
        let client = crate::anilist::client::AniListClient::new();
        let data = client.query(&crate::anilist::queries::idmal_batch_query(1), json!({ "i0": best["idMal"] })).await.unwrap();
        println!("AniList por idMal: id={} título={}", data["m0"]["id"], data["m0"]["title"]["romaji"]);
        assert_eq!(data["m0"]["id"], 20577);
    }
}
