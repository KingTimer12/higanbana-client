//! Cliente do TMDB v3 com `reqwest` direto: uma requisição por vez, ≥ 100 ms entre elas,
//! em `429` espera o `Retry-After` e tenta de novo.

use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use super::{
    TmdbError, TmdbResult, credentials,
    types::{Episode, TvHit, TvShow},
};

const BASE: &str = "https://api.themoviedb.org/3";
const MIN_INTERVAL: Duration = Duration::from_millis(100);
const MAX_ATTEMPTS: usize = 3;
const DEFAULT_RETRY: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);

pub struct TmdbClient {
    http: Client,
    token: Option<String>,
    api_key: Option<String>,
    /// Segurado durante a chamada inteira (uma por vez); guarda o início da última requisição.
    gate: Mutex<Option<Instant>>,
}

impl Default for TmdbClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct SearchPage {
    results: Vec<TvHit>,
}

#[derive(Deserialize)]
struct SeasonPage {
    episodes: Vec<Episode>,
}

impl TmdbClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(TOTAL_TIMEOUT)
                .build()
                .expect("falha ao criar cliente HTTP"),
            token: credentials::token(),
            api_key: credentials::api_key(),
            gate: Mutex::new(None),
        }
    }

    /// Há alguma credencial embutida no build?
    pub fn configured(&self) -> bool {
        self.token.is_some() || self.api_key.is_some()
    }

    pub async fn search_tv(&self, query: &str, lang: &str) -> TmdbResult<Vec<TvHit>> {
        let body = self.get("/search/tv", &[("query", query), ("language", lang)]).await?;
        parse_search(&body)
    }

    pub async fn tv(&self, id: u64, lang: &str) -> TmdbResult<TvShow> {
        let body = self.get(&format!("/tv/{id}"), &[("language", lang)]).await?;
        parse_json(&body)
    }

    pub async fn season(&self, id: u64, season: u32, lang: &str) -> TmdbResult<Vec<Episode>> {
        let body = self.get(&format!("/tv/{id}/season/{season}"), &[("language", lang)]).await?;
        parse_season(&body)
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> TmdbResult<String> {
        if !self.configured() {
            return Err(TmdbError::NotConfigured);
        }
        let mut last = self.gate.lock().await;
        for attempt in 0..MAX_ATTEMPTS {
            if let Some(prev) = *last {
                tokio::time::sleep_until((prev + MIN_INTERVAL).into()).await;
            }
            *last = Some(Instant::now());

            let mut req = self.http.get(format!("{BASE}{path}")).query(query);
            match (&self.token, &self.api_key) {
                (Some(token), _) => req = req.header(header::AUTHORIZATION, format!("Bearer {token}")),
                (None, Some(key)) => req = req.query(&[("api_key", key.as_str())]),
                (None, None) => return Err(TmdbError::NotConfigured),
            }
            let resp = req.send().await.map_err(|e| TmdbError::Http(e.without_url().to_string()))?;
            let status = resp.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                let wait = retry_after(resp.headers().get(header::RETRY_AFTER).and_then(|v| v.to_str().ok()));
                if attempt + 1 < MAX_ATTEMPTS {
                    tokio::time::sleep(wait).await;
                }
                continue;
            }
            if status == StatusCode::NOT_FOUND {
                return Err(TmdbError::NotFound);
            }
            if !status.is_success() {
                return Err(TmdbError::Http(format!("HTTP {status}")));
            }
            return resp.text().await.map_err(|e| TmdbError::Http(e.without_url().to_string()));
        }
        Err(TmdbError::RateLimited)
    }
}

/// Espera do `429`: segundos do `Retry-After` + 1 s; ausente ou data HTTP → 10 s.
fn retry_after(header: Option<&str>) -> Duration {
    header
        .and_then(|h| h.trim().parse::<u64>().ok())
        .map(|s| Duration::from_secs(s.saturating_add(1)))
        .unwrap_or(DEFAULT_RETRY)
}

fn parse_json<T: DeserializeOwned>(body: &str) -> TmdbResult<T> {
    serde_json::from_str(body).map_err(|e| TmdbError::Decode(e.to_string()))
}

fn parse_search(body: &str) -> TmdbResult<Vec<TvHit>> {
    parse_json::<SearchPage>(body).map(|p| p.results)
}

fn parse_season(body: &str) -> TmdbResult<Vec<Episode>> {
    parse_json::<SeasonPage>(body).map(|p| p.episodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_number_adds_one_second() {
        assert_eq!(retry_after(Some("3")), Duration::from_secs(4));
        assert_eq!(retry_after(Some(" 0 ")), Duration::from_secs(1));
    }

    #[test]
    fn retry_after_absent_or_date_is_ten_seconds() {
        assert_eq!(retry_after(None), Duration::from_secs(10));
        assert_eq!(retry_after(Some("Wed, 21 Oct 2026 07:28:00 GMT")), Duration::from_secs(10));
    }

    #[test]
    fn decodes_tv_show() {
        let body = r#"{"id":209867,"name":"Frieren","vote_average":8.8,"vote_count":500,
            "extra":1,"seasons":[{"season_number":0,"episode_count":4,"air_date":null},
            {"season_number":1,"episode_count":28,"air_date":"2023-09-29"}]}"#;
        let show: TvShow = parse_json(body).unwrap();
        assert_eq!(show.id, 209867);
        assert_eq!(show.seasons.len(), 2);
        assert_eq!(show.seasons[0].air_date, None);
        assert_eq!(show.seasons[1].episode_count, 28);
    }

    #[test]
    fn decodes_season_episodes_with_null_fields() {
        let body = r#"{"id":1,"episodes":[
            {"season_number":1,"episode_number":1,"name":"A","overview":"x","air_date":"2023-09-29",
             "still_path":"/a.jpg","vote_average":8.1,"vote_count":10,"runtime":25},
            {"season_number":1,"episode_number":2,"name":"B","overview":"","air_date":null,
             "still_path":null,"vote_average":0,"vote_count":0,"runtime":null},
            {"season_number":1,"episode_number":3,"name":"C","overview":"","air_date":"",
             "still_path":null,"vote_average":0.0,"vote_count":0}]}"#;
        let eps = parse_season(body).unwrap();
        assert_eq!(eps.len(), 3);
        assert_eq!(eps[0].still_path.as_deref(), Some("/a.jpg"));
        assert_eq!(eps[1].still_path, None);
        assert_eq!(eps[1].air_date, None);
        assert_eq!(eps[2].air_date.as_deref(), Some(""));
        assert_eq!(eps[2].runtime, None);
    }

    #[test]
    fn decodes_search_results() {
        let body = r#"{"page":1,"results":[{"id":209867,"name":"Frieren","original_name":"葬送のフリーレン",
            "first_air_date":"2023-09-29"},{"id":2,"name":"X","original_name":"Y","first_air_date":""}],
            "total_results":2}"#;
        let hits = parse_search(body).unwrap();
        assert_eq!(hits[0].id, 209867);
        assert_eq!(hits[1].first_air_date.as_deref(), Some(""));
    }

    #[test]
    fn bad_json_is_decode_error() {
        assert!(matches!(parse_search("{}"), Err(TmdbError::Decode(_))));
    }

    #[tokio::test]
    async fn unconfigured_client_fails_without_network() {
        let mut client = TmdbClient::new();
        client.token = None;
        client.api_key = None;
        assert!(!client.configured());
        assert!(matches!(client.search_tv("x", "pt-BR").await, Err(TmdbError::NotConfigured)));
    }

    /// Rede real; precisa de `TMDB_TOKEN`/`TMDB_KEY` no build. `cargo test -- --ignored live_frieren`.
    #[tokio::test]
    #[ignore]
    async fn live_frieren() {
        let client = TmdbClient::new();
        let hits = client.search_tv("Sousou no Frieren", "pt-BR").await.unwrap();
        assert!(hits.iter().any(|h| h.id == 209867));
        let eps = client.season(209867, 1, "pt-BR").await.unwrap();
        assert_eq!(eps.len(), 38);
        assert!(eps.iter().all(|e| e.still_path.is_some()));
    }
}
