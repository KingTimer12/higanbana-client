use std::{
    sync::{
        Mutex as StdMutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use reqwest::{Client, StatusCode, header::{self, HeaderMap}};
use tokio::sync::Mutex;

use super::error::{AniListError, AniListResult};

const ENDPOINT: &str = "https://graphql.anilist.co";
const MAX_ATTEMPTS: usize = 3;
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(60);
/// Pedidos por minuto até o AniList dizer outro valor (`X-RateLimit-Limit`); hoje roda degradado em 30.
const DEFAULT_LIMIT: u32 = 30;
/// Intervalo mínimo entre pedidos do que a pessoa está vendo.
const INTERACTIVE_GAP: Duration = Duration::from_millis(700);
/// Folga somada ao `Retry-After` de um 429.
const RETRY_SLACK: Duration = Duration::from_secs(1);
/// De quanto em quanto tempo um pedido em segundo plano olha se alguém passou na frente.
const YIELD_STEP: Duration = Duration::from_millis(200);

/// Quem está pedindo: decide o ritmo e a folga deixada no limite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// O que a pessoa está vendo (cards, detalhe, franquia).
    Interactive,
    /// Rastreio de metadados: anda devagar, deixa folga e cede a vez.
    Background,
}

/// O que se sabe do limite do AniList.
#[derive(Debug, Clone, Copy)]
struct LimitState {
    limit: u32,
    remaining: Option<u32>,
    reset_at: Option<Instant>,
    blocked_until: Option<Instant>,
    last_sent: Option<Instant>,
}

impl Default for LimitState {
    fn default() -> Self {
        Self { limit: DEFAULT_LIMIT, remaining: None, reset_at: None, blocked_until: None, last_sent: None }
    }
}

impl LimitState {
    /// Aplica os cabeçalhos de uma resposta recebida em `now`.
    fn absorb(&mut self, h: &RateHeaders, now: Instant, too_many: bool) {
        if let Some(limit) = h.limit {
            self.limit = limit.max(1);
        }
        if let Some(remaining) = h.remaining {
            self.remaining = Some(remaining);
        }
        if let Some(reset_in) = h.reset_in {
            self.reset_at = Some(now + reset_in);
        }
        if too_many {
            self.blocked_until = Some(now + h.retry_after.unwrap_or(DEFAULT_RETRY_AFTER) + RETRY_SLACK);
            self.remaining = Some(0);
        }
    }
}

/// Cabeçalhos de limite de uma resposta.
#[derive(Debug, Default, PartialEq)]
struct RateHeaders {
    limit: Option<u32>,
    remaining: Option<u32>,
    /// Tempo até a janela do limite recomeçar (`X-RateLimit-Reset` é epoch em segundos).
    reset_in: Option<Duration>,
    retry_after: Option<Duration>,
}

fn parse_rate_headers(headers: &HeaderMap, now_epoch: u64) -> RateHeaders {
    let number = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|v| v.max(0) as u64)
    };
    RateHeaders {
        limit: number("x-ratelimit-limit").map(|v| v.min(u32::MAX as u64) as u32),
        remaining: number("x-ratelimit-remaining").map(|v| v.min(u32::MAX as u64) as u32),
        reset_in: number("x-ratelimit-reset").map(|at| Duration::from_secs(at.saturating_sub(now_epoch))),
        retry_after: number(header::RETRY_AFTER.as_str()).map(Duration::from_secs),
    }
}

/// Quando um pedido com `priority` pode sair, dado o que se sabe do limite em `now`.
///
/// Nunca antes do bloqueio de um 429; respeita o intervalo mínimo desde o último pedido; e, se o
/// que sobra na janela já chegou na reserva da prioridade, espera a janela recomeçar.
/// `Background` reserva um terço do limite para o que a pessoa está vendo, também no intervalo
/// (`60 s / (limite - reserva)`), para valer mesmo sem saber quando a janela recomeça.
fn send_at(state: &LimitState, now: Instant, priority: Priority) -> Instant {
    let limit = state.limit.max(1);
    let (gap, reserve) = match priority {
        Priority::Interactive => (INTERACTIVE_GAP, 1),
        // O ritmo já deixa a reserva livre: o AniList quase nunca manda `X-RateLimit-Reset` fora do 429.
        Priority::Background => {
            let reserve = limit / 3;
            (Duration::from_secs(60) / (limit - reserve).max(1), reserve)
        }
    };
    let mut at = now;
    if let Some(until) = state.blocked_until {
        at = at.max(until);
    }
    if let Some(last) = state.last_sent {
        at = at.max(last + gap);
    }
    if let (Some(remaining), Some(reset)) = (state.remaining, state.reset_at)
        && remaining <= reserve
        && reset > now
    {
        at = at.max(reset);
    }
    at
}

/// Conta um pedido `Interactive` esperando enquanto vive.
struct Waiting<'a>(&'a AtomicUsize);

impl<'a> Waiting<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Cliente GraphQL do AniList (público, sem auth).
///
/// Tem ritmo próprio: uma requisição por vez, espaçadas conforme a prioridade e os cabeçalhos
/// `X-RateLimit-*`. Em `429` espera o `Retry-After` antes de tentar de novo, e todo mundo espera junto.
pub struct AniListClient {
    http: Client,
    /// Vez de mandar: segurada da espera até a resposta (uma requisição em voo).
    gate: Mutex<()>,
    state: StdMutex<LimitState>,
    /// Pedidos `Interactive` esperando; enquanto houver algum, `Background` cede a vez.
    interactive_waiting: AtomicUsize,
}

impl AniListClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("falha ao criar cliente HTTP"),
            gate: Mutex::new(()),
            state: StdMutex::new(LimitState::default()),
            interactive_waiting: AtomicUsize::new(0),
        }
    }

    /// Executa query com prioridade `Interactive`; ver [`Self::query_with`].
    pub async fn query(&self, query: &str, variables: Value) -> AniListResult<Value> {
        self.query_with(Priority::Interactive, query, variables).await
    }

    /// Executa query e devolve `data`. Erros parciais (ex.: um alias 404) são ignorados
    /// quando há `data`; sem `data`, o primeiro erro vira [`AniListError::Api`].
    pub async fn query_with(&self, priority: Priority, query: &str, variables: Value) -> AniListResult<Value> {
        let body = json!({ "query": query, "variables": variables }).to_string();

        for _ in 0..MAX_ATTEMPTS {
            let (status, text) = self.send(priority, &body).await?;
            if status == StatusCode::TOO_MANY_REQUESTS {
                continue;
            }
            let parsed: Value = serde_json::from_str(&text)
                .map_err(|e| AniListError::Decode(format!("{e} (HTTP {status})")))?;
            return extract_data(parsed);
        }
        Err(AniListError::RateLimited)
    }

    /// Fim da pausa pedida pelo AniList (429), se ainda estiver valendo.
    pub fn paused_until(&self) -> Option<Instant> {
        let now = Instant::now();
        self.state.lock().unwrap().blocked_until.filter(|&until| until > now)
    }

    /// Espera a vez e o ritmo, manda o pedido e lê os cabeçalhos de limite.
    async fn send(&self, priority: Priority, body: &str) -> reqwest::Result<(StatusCode, String)> {
        let _waiting = (priority == Priority::Interactive).then(|| Waiting::new(&self.interactive_waiting));
        let _gate = loop {
            let gate = self.gate.lock().await;
            let now = Instant::now();
            if priority == Priority::Background && self.interactive_waiting.load(Ordering::SeqCst) > 0 {
                drop(gate);
                tokio::time::sleep(YIELD_STEP).await;
                continue;
            }
            let at = send_at(&self.state.lock().unwrap(), now, priority);
            if at <= now {
                break gate;
            }
            match priority {
                // Só quem tem a vez muda o estado: depois da espera, pode sair.
                Priority::Interactive => {
                    tokio::time::sleep_until(at.into()).await;
                    break gate;
                }
                // Espera sem a vez, em passos curtos, para um `Interactive` poder passar na frente.
                Priority::Background => {
                    drop(gate);
                    tokio::time::sleep_until(at.min(now + YIELD_STEP).into()).await;
                }
            }
        };

        self.state.lock().unwrap().last_sent = Some(Instant::now());
        let resp = self
            .http
            .post(ENDPOINT)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json")
            .body(body.to_owned())
            .send()
            .await?;
        let status = resp.status();
        let limits = parse_rate_headers(resp.headers(), epoch_now());
        self.state.lock().unwrap().absorb(&limits, Instant::now(), status == StatusCode::TOO_MANY_REQUESTS);
        let text = resp.text().await?;
        Ok((status, text))
    }
}

fn epoch_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

fn extract_data(mut body: Value) -> AniListResult<Value> {
    let data = body.get_mut("data").map(Value::take).unwrap_or(Value::Null);
    let has_data = data.as_object().is_some_and(|o| o.values().any(|v| !v.is_null()));
    if has_data {
        return Ok(data);
    }
    let first_error = body
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|errs| errs.first());
    match first_error {
        Some(err) => {
            let status = err.get("status").and_then(Value::as_u64).unwrap_or(500) as u16;
            if status == 404 {
                return Err(AniListError::NotFound);
            }
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("erro desconhecido")
                .to_owned();
            Err(AniListError::Api { status, message })
        }
        None => Err(AniListError::NotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_not_found() {
        let body = json!({"errors":[{"message":"Not Found.","status":404}],"data":{"Media":null}});
        assert!(matches!(extract_data(body), Err(AniListError::NotFound)));
    }

    #[test]
    fn keeps_partial_data() {
        let body = json!({"errors":[{"message":"x","status":404}],"data":{"a":null,"b":{"id":1}}});
        assert_eq!(extract_data(body).unwrap()["b"]["id"], 1);
    }

    #[test]
    fn maps_api_error() {
        let body = json!({"errors":[{"message":"Max query complexity","status":400}],"data":null});
        assert!(matches!(extract_data(body), Err(AniListError::Api { status: 400, .. })));
    }

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn fresh_state_sends_now() {
        let now = Instant::now();
        let st = LimitState::default();
        assert_eq!(send_at(&st, now, Priority::Interactive), now);
        assert_eq!(send_at(&st, now, Priority::Background), now);
    }

    #[test]
    fn waits_for_block() {
        let now = Instant::now();
        let st = LimitState { blocked_until: Some(now + 5 * SEC), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Interactive), now + 5 * SEC);
        assert_eq!(send_at(&st, now, Priority::Background), now + 5 * SEC);
        // Bloqueio vencido não segura nada.
        let st = LimitState { blocked_until: Some(now), ..Default::default() };
        assert_eq!(send_at(&st, now + SEC, Priority::Interactive), now + SEC);
    }

    #[test]
    fn gap_depends_on_priority() {
        let now = Instant::now();
        let st = LimitState { last_sent: Some(now), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Interactive), now + Duration::from_millis(700));
        // Fundo: 60 s / (limite - reserva) = 60 / 20 = 3 s, deixando ~10/min para o Interactive.
        assert_eq!(send_at(&st, now, Priority::Background), now + 3 * SEC);
        // Limite maior, ritmo de fundo mais rápido: 60 / (90 - 30) = 1 s.
        let st = LimitState { limit: 90, last_sent: Some(now), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Background), now + SEC);
        // Limite minúsculo não divide por zero.
        let st = LimitState { limit: 1, last_sent: Some(now), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Background), now + 60 * SEC);
        // Intervalo já passado: sai na hora.
        let st = LimitState { last_sent: Some(now), ..Default::default() };
        assert_eq!(send_at(&st, now + 3 * SEC, Priority::Background), now + 3 * SEC);
    }

    #[test]
    fn reserve_holds_background_not_interactive() {
        let now = Instant::now();
        let reset = now + 20 * SEC;
        let st = LimitState { remaining: Some(10), reset_at: Some(reset), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Background), reset);
        assert_eq!(send_at(&st, now, Priority::Interactive), now);
        // Acima da reserva (10 com limite 30), o fundo segue.
        let st = LimitState { remaining: Some(11), reset_at: Some(reset), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Background), now);
        // Na última vaga, até o Interactive espera o reset.
        let st = LimitState { remaining: Some(1), reset_at: Some(reset), ..Default::default() };
        assert_eq!(send_at(&st, now, Priority::Interactive), reset);
        // Reset já passou: o `remaining` antigo não vale mais.
        let st = LimitState { remaining: Some(0), reset_at: Some(now), ..Default::default() };
        assert_eq!(send_at(&st, now + SEC, Priority::Background), now + SEC);
    }

    #[test]
    fn background_keeps_headroom_without_reset() {
        let now = Instant::now();
        // O AniList quase nunca manda `X-RateLimit-Reset` fora do 429: sem ele, o intervalo segura a folga.
        let st = LimitState { remaining: Some(5), reset_at: None, last_sent: Some(now), ..Default::default() };
        assert!(send_at(&st, now, Priority::Background) >= now + 3 * SEC);
        assert_eq!(send_at(&st, now, Priority::Interactive), now + Duration::from_millis(700));
    }

    #[test]
    fn parses_rate_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-ratelimit-limit", "30".parse().unwrap());
        h.insert("x-ratelimit-remaining", "7".parse().unwrap());
        h.insert("x-ratelimit-reset", "1000012".parse().unwrap());
        h.insert(header::RETRY_AFTER, "11".parse().unwrap());
        assert_eq!(
            parse_rate_headers(&h, 1_000_000),
            RateHeaders {
                limit: Some(30),
                remaining: Some(7),
                reset_in: Some(12 * SEC),
                retry_after: Some(11 * SEC),
            }
        );
        // Reset no passado vira zero; remaining negativo vira zero; lixo é ignorado.
        let mut h = HeaderMap::new();
        h.insert("x-ratelimit-remaining", "-2".parse().unwrap());
        h.insert("x-ratelimit-reset", "999990".parse().unwrap());
        h.insert("x-ratelimit-limit", "abc".parse().unwrap());
        assert_eq!(
            parse_rate_headers(&h, 1_000_000),
            RateHeaders { limit: None, remaining: Some(0), reset_in: Some(Duration::ZERO), retry_after: None }
        );
        assert_eq!(parse_rate_headers(&HeaderMap::new(), 1_000_000), RateHeaders::default());
    }

    #[test]
    fn absorbs_headers_and_429() {
        let now = Instant::now();
        let mut st = LimitState::default();
        let h = RateHeaders { limit: Some(90), remaining: Some(40), reset_in: Some(30 * SEC), retry_after: None };
        st.absorb(&h, now, false);
        assert_eq!((st.limit, st.remaining, st.reset_at, st.blocked_until), (90, Some(40), Some(now + 30 * SEC), None));

        let h = RateHeaders { retry_after: Some(11 * SEC), ..Default::default() };
        st.absorb(&h, now, true);
        assert_eq!(st.blocked_until, Some(now + 12 * SEC));
        assert_eq!(st.remaining, Some(0));

        // 429 sem `Retry-After`: espera o padrão.
        let mut st = LimitState::default();
        st.absorb(&RateHeaders::default(), now, true);
        assert_eq!(st.blocked_until, Some(now + DEFAULT_RETRY_AFTER + SEC));
        // Limite zero não quebra a conta do ritmo.
        st.absorb(&RateHeaders { limit: Some(0), ..Default::default() }, now, false);
        assert_eq!(st.limit, 1);
    }
}
