//! Cliente do fanart.tv: até 4 requisições em paralelo, 404 vira "sem imagens".

use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use tokio::sync::Semaphore;

use super::{
    FanartError, FanartResult, credentials,
    pick::{MovieImages, TvImages},
};

const BASE: &str = "https://webservice.fanart.tv";
const PERMITS: usize = 4;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const JSON_TIMEOUT: Duration = Duration::from_secs(10);
const IMAGE_TIMEOUT: Duration = Duration::from_secs(20);

pub struct FanartClient {
    http: Client,
    api_key: Option<String>,
    /// Uma permissão segurada durante cada requisição.
    gate: Semaphore,
}

impl Default for FanartClient {
    fn default() -> Self {
        Self::new()
    }
}

impl FanartClient {
    pub fn new() -> Self {
        Self::with_key(credentials::api_key())
    }

    fn with_key(api_key: Option<String>) -> Self {
        Self {
            http: Client::builder()
                .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(CONNECT_TIMEOUT)
                .build()
                .expect("falha ao criar cliente HTTP"),
            api_key,
            gate: Semaphore::new(PERMITS),
        }
    }

    /// Há chave embutida no build?
    pub fn configured(&self) -> bool {
        self.api_key.is_some()
    }

    pub async fn tv(&self, tvdb: u32) -> FanartResult<TvImages> {
        self.json(&format!("/v3/tv/{tvdb}")).await
    }

    pub async fn movie(&self, tmdb: u32) -> FanartResult<MovieImages> {
        self.json(&format!("/v3/movies/{tmdb}")).await
    }

    /// Baixa uma imagem; só `https`.
    pub async fn download(&self, url: &str) -> FanartResult<Vec<u8>> {
        if !url.starts_with("https://") {
            return Err(FanartError::Http("endereço não permitido".into()));
        }
        let _permit = self.gate.acquire().await.map_err(|e| FanartError::Http(e.to_string()))?;
        let resp = self.http.get(url).timeout(IMAGE_TIMEOUT).send().await.map_err(http_err)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(FanartError::Http(format!("HTTP {status}")));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(http_err)
    }

    async fn json<T: DeserializeOwned + Default>(&self, path: &str) -> FanartResult<T> {
        let key = self.api_key.as_deref().ok_or(FanartError::NotConfigured)?;
        let _permit = self.gate.acquire().await.map_err(|e| FanartError::Http(e.to_string()))?;
        let resp = self
            .http
            .get(format!("{BASE}{path}"))
            .query(&[("api_key", key)])
            .timeout(JSON_TIMEOUT)
            .send()
            .await
            .map_err(http_err)?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(T::default());
        }
        if !status.is_success() {
            return Err(FanartError::Http(format!("HTTP {status}")));
        }
        let body = resp.text().await.map_err(http_err)?;
        serde_json::from_str(&body).map_err(|e| FanartError::Decode(e.to_string()))
    }
}

/// Sem a URL no texto: ela leva a chave.
fn http_err(e: reqwest::Error) -> FanartError {
    FanartError::Http(e.without_url().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unconfigured_never_hits_network() {
        let client = FanartClient::with_key(None);
        assert!(!client.configured());
        assert!(matches!(client.tv(1).await, Err(FanartError::NotConfigured)));
        assert!(matches!(client.movie(1).await, Err(FanartError::NotConfigured)));
    }

    #[tokio::test]
    async fn download_rejects_non_https() {
        let client = FanartClient::with_key(None);
        assert!(matches!(client.download("http://assets.fanart.tv/x.jpg").await, Err(FanartError::Http(_))));
    }

    #[test]
    fn decodes_real_shape() {
        let body = r#"{"name":"X","tvposter":[{"id":"1","url":"https://a/b.jpg","lang":"en","likes":"6"}],
            "seasonposter":[{"id":"2","url":"https://a/c.jpg","lang":"en","likes":"1","season":"2"}]}"#;
        let t: TvImages = serde_json::from_str(body).unwrap();
        assert_eq!(t.seasonposter[0].season.as_deref(), Some("2"));
    }
}
