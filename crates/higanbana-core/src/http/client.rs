use serde::{Deserialize, Serialize, de::DeserializeOwned};
use reqwest::{
    Client, Method, RequestBuilder, Response, StatusCode, Url,
    header::{CONTENT_TYPE, COOKIE},
};

use super::error::{HttpError, HttpResult};
use crate::auth::{AuthManager, config::CONFIG};

/// Como a sessão vai na requisição.
#[derive(Debug, Clone, Copy)]
pub enum AuthStyle {
    /// Cookie `sb-auth-auth-token` (Next.js do site e backend da nuvem).
    Cookie,
    /// `apikey` + `Authorization: Bearer` (APIs do Supabase).
    Bearer,
}

/// Serviços conhecidos. O frontend só consegue falar com estes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Site,
    Nuvem,
    Supabase,
}

/// Cliente HTTP de um serviço do Anitsu: base URL fixa + autenticação automática.
///
/// - Injeta a sessão (cookie ou bearer) em toda requisição.
/// - Em `401`, renova o token uma vez e repete.
/// - Status fora de 2xx vira [`HttpError::Status`].
pub struct AnitsuHttp {
    client: Client,
    base: Url,
    auth_style: AuthStyle,
}

impl AnitsuHttp {
    pub fn new(client: Client, base: &str, auth_style: AuthStyle) -> Self {
        let base = if base.ends_with('/') { base.to_owned() } else { format!("{base}/") };
        Self {
            client,
            base: base.parse().expect("base URL inválida"),
            auth_style,
        }
    }

    /// Resolve `path` relativo à base. Rejeita URLs absolutas e `..` que escapem da base.
    pub fn url(&self, path: &str) -> HttpResult<Url> {
        let url = self
            .base
            .join(path.trim_start_matches('/'))
            .map_err(|e| HttpError::InvalidPath(e.to_string()))?;
        if !url.as_str().starts_with(self.base.as_str()) {
            return Err(HttpError::InvalidPath(path.to_owned()));
        }
        Ok(url)
    }

    /// Envia requisição autenticada. `build` customiza (query, body, headers).
    pub async fn send<F>(
        &self,
        auth: &AuthManager,
        method: Method,
        path: &str,
        build: F,
    ) -> HttpResult<Response>
    where
        F: Fn(RequestBuilder) -> RequestBuilder,
    {
        let resp = self.send_raw(auth, method.clone(), path, build).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!(%method, path, status = status.as_u16(), "resposta com erro");
            return Err(HttpError::Status { status: status.as_u16(), body });
        }
        Ok(resp)
    }

    /// Como [`Self::send`] (inclusive renovar a sessão em 401), mas devolve a resposta
    /// com qualquer status, para quem precisa repassá-lo (ex.: proxy de stream: 206/416/404).
    pub async fn send_raw<F>(
        &self,
        auth: &AuthManager,
        method: Method,
        path: &str,
        build: F,
    ) -> HttpResult<Response>
    where
        F: Fn(RequestBuilder) -> RequestBuilder,
    {
        let url = self.url(path)?;
        let mut resp = self.attempt(auth, &method, &url, &build).await?;
        if resp.status() == StatusCode::UNAUTHORIZED && auth.is_logged_in().await {
            auth.refresh().await?;
            resp = self.attempt(auth, &method, &url, &build).await?;
        }
        Ok(resp)
    }

    async fn attempt<F>(
        &self,
        auth: &AuthManager,
        method: &Method,
        url: &Url,
        build: &F,
    ) -> HttpResult<Response>
    where
        F: Fn(RequestBuilder) -> RequestBuilder,
    {
        let started = std::time::Instant::now();
        let mut req = self.client.request(method.clone(), url.clone());
        req = match self.auth_style {
            AuthStyle::Cookie => match auth.cookie_header().await? {
                Some(cookie) => req.header(COOKIE, cookie),
                None => req,
            },
            AuthStyle::Bearer => {
                let token = auth.access_token().await?;
                req.header("apikey", &CONFIG.supabase_anon_key)
                    .bearer_auth(token.as_deref().unwrap_or(&CONFIG.supabase_anon_key))
            }
        };
        // Só host e caminho: a query pode carregar dados da sessão.
        match build(req).send().await {
            Ok(resp) => {
                tracing::debug!(%method, host = url.host_str().unwrap_or(""), path = url.path(), status = resp.status().as_u16(), ms = started.elapsed().as_millis() as u64, "http");
                Ok(resp)
            }
            Err(e) => {
                let e = e.without_url();
                tracing::warn!(%method, host = url.host_str().unwrap_or(""), path = url.path(), error = %e, "http falhou");
                Err(e.into())
            }
        }
    }

    /// GET com query string, resposta em JSON tipado.
    pub async fn get_json<T, Q>(&self, auth: &AuthManager, path: &str, query: &Q) -> HttpResult<T>
    where
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let resp = self.send(auth, Method::GET, path, |r| r.query(query)).await?;
        resp.json::<T>().await.map_err(|e| HttpError::Decode(e.to_string()))
    }
}

/// Lê a resposta como JSON se for JSON, senão como texto. Vazio = `null`.
pub async fn read_value(resp: Response) -> HttpResult<serde_json::Value> {
    let is_json = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.contains("json"));
    let text = resp.text().await?;
    if text.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    if is_json {
        serde_json::from_str(&text).map_err(|e| HttpError::Decode(e.to_string()))
    } else {
        Ok(serde_json::Value::String(text))
    }
}

/// Clientes de todos os serviços, registrados como state do Tauri.
pub struct AnitsuApis {
    pub site: AnitsuHttp,
    pub nuvem: AnitsuHttp,
    pub supabase: AnitsuHttp,
}

impl AnitsuApis {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("falha ao criar cliente HTTP");
        Self {
            site: AnitsuHttp::new(client.clone(), &CONFIG.site_url, AuthStyle::Cookie),
            nuvem: AnitsuHttp::new(client.clone(), &format!("{}/api", CONFIG.nuvem_url), AuthStyle::Cookie),
            supabase: AnitsuHttp::new(client, &format!("{}/rest/v1", CONFIG.supabase_url), AuthStyle::Bearer),
        }
    }

    pub fn get(&self, service: Service) -> &AnitsuHttp {
        match service {
            Service::Site => &self.site,
            Service::Nuvem => &self.nuvem,
            Service::Supabase => &self.supabase,
        }
    }
}

impl Default for AnitsuApis {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http() -> AnitsuHttp {
        AnitsuHttp::new(Client::new(), "https://nuvem.anitsu.moe/api", AuthStyle::Cookie)
    }

    #[test]
    fn joins_relative_paths() {
        let h = http();
        assert_eq!(h.url("files").unwrap().as_str(), "https://nuvem.anitsu.moe/api/files");
        assert_eq!(h.url("/files").unwrap().as_str(), "https://nuvem.anitsu.moe/api/files");
    }

    #[test]
    fn rejects_escaping_paths() {
        let h = http();
        assert!(h.url("https://evil.com/x").is_err());
        // Barras iniciais são removidas: vira caminho relativo, continua dentro da base.
        assert_eq!(
            h.url("//evil.com/x").unwrap().as_str(),
            "https://nuvem.anitsu.moe/api/evil.com/x"
        );
        assert!(h.url("../secret").is_err());
    }
}
