//! Proxy HTTP local entre o mpv e a nuvem.
//!
//! O mpv abre `http://127.0.0.1:<porta>/<token>/stream?path=<caminho>` e pede
//! pedaços com `Range`. O proxy repassa cada pedido para a nuvem com a sessão,
//! que só existe no Rust, e devolve o corpo em stream, sem guardar nada.
//! - escuta só em 127.0.0.1, com token aleatório no caminho;
//! - no máximo [`MAX_UPSTREAM_PER_FILE`] conexões com a nuvem por arquivo;
//! - quando o mpv fecha a conexão, o corpo é descartado e a requisição de cima cai junto.

use std::{
    collections::HashMap,
    convert::Infallible,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use futures_util::{Stream, StreamExt, future::BoxFuture};
use http_body_util::{BodyExt, Full, StreamBody, combinators::UnsyncBoxBody};
use hyper::{Request, Response, StatusCode, body::Frame, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, sync::Semaphore};

pub const MAX_UPSTREAM_PER_FILE: usize = 2;
/// Miniaturas têm faixa própria: não tiram vaga do vídeo que está tocando.
/// Duas, e não uma: o FFmpeg abre a conexão do `seek` antes de fechar a antiga.
pub const MAX_UPSTREAM_PER_PREVIEW: usize = 2;

/// Quem pede: o player ou o gerador de miniaturas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Lane {
    Player,
    Preview,
}

impl Lane {
    fn limit(self) -> usize {
        match self {
            Lane::Player => MAX_UPSTREAM_PER_FILE,
            Lane::Preview => MAX_UPSTREAM_PER_PREVIEW,
        }
    }
}

/// Headers da nuvem que chegam ao mpv; o resto é descartado.
const FORWARDED_HEADERS: [&str; 4] = ["content-type", "content-length", "content-range", "accept-ranges"];

pub type BodyStream = Pin<Box<dyn Stream<Item = Result<Bytes, io::Error>> + Send>>;

pub struct UpstreamResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: BodyStream,
}

/// De onde o proxy busca os bytes (a nuvem, ou um falso nos testes).
pub trait Upstream: Send + Sync + 'static {
    fn fetch(&self, path: &str, range: Option<&str>) -> BoxFuture<'static, Result<UpstreamResponse, String>>;
}

pub struct StreamProxy {
    port: u16,
    token: String,
}

type ProxyBody = UnsyncBoxBody<Bytes, io::Error>;

struct Shared<U> {
    upstream: U,
    token: String,
    limits: Mutex<HashMap<(String, Lane), Arc<Semaphore>>>,
}

impl StreamProxy {
    /// Sobe o servidor numa porta aleatória de 127.0.0.1 (precisa de runtime tokio).
    pub async fn start<U: Upstream>(upstream: U) -> io::Result<StreamProxy> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
        let port = listener.local_addr()?.port();
        let token = random_token()?;
        let shared = Arc::new(Shared { upstream, token: token.clone(), limits: Mutex::default() });

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { continue };
                let shared = shared.clone();
                tokio::spawn(async move {
                    let service = service_fn(move |req| handle(shared.clone(), req));
                    let _ = http1::Builder::new().serve_connection(TokioIo::new(stream), service).await;
                });
            }
        });
        Ok(StreamProxy { port, token })
    }

    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url_for(&self, nuvem_path: &str) -> String {
        format!("http://127.0.0.1:{}/{}/stream?path={}", self.port, self.token, urlencoding::encode(nuvem_path))
    }

    /// URL para o gerador de miniaturas: mesmo arquivo, faixa de conexões própria.
    pub fn url_for_preview(&self, nuvem_path: &str) -> String {
        format!("{}&lane=preview", self.url_for(nuvem_path))
    }
}

fn random_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn plain(status: StatusCode) -> Response<ProxyBody> {
    let mut resp = Response::new(Full::new(Bytes::new()).map_err(|never: Infallible| match never {}).boxed_unsync());
    *resp.status_mut() = status;
    resp
}

/// `/<token>/stream?path=<caminho>[&lane=preview]` → caminho e faixa, se o token confere.
fn requested_path(uri: &hyper::Uri, token: &str) -> Option<(String, Lane)> {
    if uri.path() != format!("/{token}/stream") {
        return None;
    }
    let query = uri.query()?;
    let raw = query.split('&').find_map(|kv| kv.strip_prefix("path="))?;
    let lane = if query.split('&').any(|kv| kv == "lane=preview") { Lane::Preview } else { Lane::Player };
    urlencoding::decode(raw).ok().map(|p| (p.into_owned(), lane))
}

async fn handle<U: Upstream>(
    shared: Arc<Shared<U>>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<ProxyBody>, Infallible> {
    let Some((path, lane)) = requested_path(req.uri(), &shared.token) else {
        return Ok(plain(StatusCode::FORBIDDEN));
    };
    let range = req.headers().get(hyper::header::RANGE).and_then(|v| v.to_str().ok()).map(str::to_owned);

    let limit = shared
        .limits
        .lock()
        .unwrap()
        .entry((path.clone(), lane))
        .or_insert_with(|| Arc::new(Semaphore::new(lane.limit())))
        .clone();
    let Ok(permit) = limit.acquire_owned().await else {
        return Ok(plain(StatusCode::SERVICE_UNAVAILABLE));
    };

    let up = match shared.upstream.fetch(&path, range.as_deref()).await {
        Ok(up) => up,
        Err(_) => return Ok(plain(StatusCode::BAD_GATEWAY)),
    };

    // A permissão vive junto com o corpo: solta quando o mpv termina ou desiste.
    let frames = up.body.map(move |chunk| {
        let _held = &permit;
        chunk.map(Frame::data)
    });
    let mut resp = Response::new(StreamBody::new(frames).boxed_unsync());
    *resp.status_mut() = StatusCode::from_u16(up.status).unwrap_or(StatusCode::BAD_GATEWAY);
    for (name, value) in up.headers {
        if FORWARDED_HEADERS.contains(&name.to_ascii_lowercase().as_str())
            && let (Ok(n), Ok(v)) = (hyper::header::HeaderName::try_from(name), hyper::header::HeaderValue::try_from(value))
        {
            resp.headers_mut().insert(n, v);
        }
    }
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use bytes::Bytes;
    use futures_util::{FutureExt, future::BoxFuture, stream};
    use reqwest::{Client, StatusCode, header::RANGE};

    use super::*;

    const SIZE: usize = 1024 * 1024;

    /// Conta conexões ativas: incrementa no `fetch`, decrementa quando o corpo é descartado.
    struct ActiveGuard(Arc<AtomicUsize>);
    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[derive(Clone, Default)]
    struct Fake {
        data: Arc<Vec<u8>>,
        active: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
        last_path: Arc<Mutex<Option<String>>>,
        start_delay: Duration,
        /// Libera o corpo em pedaços de 64 KiB com esse intervalo.
        chunk_delay: Option<Duration>,
        /// Responde com este status (ex.: 416, 404) em vez do conteúdo.
        status_override: Option<u16>,
    }

    impl Fake {
        fn new() -> Self {
            Self { data: Arc::new((0..SIZE).map(|i| (i % 251) as u8).collect()), ..Default::default() }
        }
    }

    fn parse_range(range: Option<&str>, len: usize) -> (usize, usize) {
        let Some(r) = range.and_then(|r| r.strip_prefix("bytes=")) else { return (0, len - 1) };
        let (a, b) = r.split_once('-').unwrap();
        let a: usize = a.parse().unwrap();
        let b: usize = if b.is_empty() { len - 1 } else { b.parse().unwrap() };
        (a, b.min(len - 1))
    }

    impl Upstream for Fake {
        fn fetch(&self, path: &str, range: Option<&str>) -> BoxFuture<'static, Result<UpstreamResponse, String>> {
            let me = self.clone();
            let path = path.to_owned();
            let range = range.map(str::to_owned);
            async move {
                me.calls.fetch_add(1, Ordering::SeqCst);
                let now = me.active.fetch_add(1, Ordering::SeqCst) + 1;
                me.peak.fetch_max(now, Ordering::SeqCst);
                let guard = ActiveGuard(me.active.clone());
                *me.last_path.lock().unwrap() = Some(path);
                tokio::time::sleep(me.start_delay).await;
                if let Some(status) = me.status_override {
                    return Ok(UpstreamResponse {
                        status,
                        headers: vec![("Content-Range".into(), format!("bytes */{}", me.data.len()))],
                        body: Box::pin(stream::once(async move {
                            drop(guard);
                            Ok(Bytes::new())
                        })),
                    });
                }

                let (a, b) = parse_range(range.as_deref(), me.data.len());
                let slice = me.data[a..=b].to_vec();
                let chunks: Vec<Bytes> = slice.chunks(64 * 1024).map(Bytes::copy_from_slice).collect();
                let delay = me.chunk_delay;
                let body = stream::unfold((chunks.into_iter(), guard), move |(mut it, g)| async move {
                    if let Some(d) = delay {
                        tokio::time::sleep(d).await;
                    }
                    it.next().map(|c| (Ok(c), (it, g)))
                });
                Ok(UpstreamResponse {
                    status: if range.is_some() { 206 } else { 200 },
                    headers: vec![
                        ("Content-Type".into(), "video/mp4".into()),
                        ("Content-Length".into(), (b - a + 1).to_string()),
                        ("Content-Range".into(), format!("bytes {a}-{b}/{}", me.data.len())),
                        ("Accept-Ranges".into(), "bytes".into()),
                        ("X-Secret".into(), "não repassar".into()),
                    ],
                    body: Box::pin(body),
                })
            }
            .boxed()
        }
    }

    #[tokio::test]
    async fn serves_range() {
        let fake = Fake::new();
        let proxy = StreamProxy::start(fake.clone()).await.unwrap();
        let resp = Client::new()
            .get(proxy.url_for("a/b.mkv"))
            .header(RANGE, "bytes=100-199")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(resp.headers()["content-range"], "bytes 100-199/1048576");
        assert!(resp.headers().get("x-secret").is_none());
        let body = resp.bytes().await.unwrap();
        assert_eq!(&body[..], &fake.data[100..200]);
    }

    #[tokio::test]
    async fn passes_through_error_status() {
        for status in [404u16, 416] {
            let fake = Fake { status_override: Some(status), ..Fake::new() };
            let proxy = StreamProxy::start(fake).await.unwrap();
            let resp = Client::new()
                .get(proxy.url_for("ep.mkv"))
                .header(RANGE, "bytes=9999999-")
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status().as_u16(), status);
        }
    }

    #[tokio::test]
    async fn rejects_without_token() {
        let fake = Fake::new();
        let proxy = StreamProxy::start(fake.clone()).await.unwrap();
        let resp = Client::new()
            .get(format!("http://127.0.0.1:{}/errado/stream?path=x", proxy.port()))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn url_roundtrip_special_chars() {
        let fake = Fake::new();
        let proxy = StreamProxy::start(fake.clone()).await.unwrap();
        let path = "Animes/Letra F/Fairy Gone/[FênixFansub_&_IsekaiSubs]_Fairy_Gone_01_[HDTV] #1 'x' 100%+1.mp4";
        let resp = Client::new()
            .get(proxy.url_for(path))
            .header(RANGE, "bytes=0-9")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(fake.last_path.lock().unwrap().as_deref(), Some(path));
    }

    #[tokio::test]
    async fn caps_upstream_concurrency() {
        let fake = Fake { start_delay: Duration::from_millis(200), ..Fake::new() };
        let proxy = Arc::new(StreamProxy::start(fake.clone()).await.unwrap());
        let client = Client::new();
        let tasks: Vec<_> = (0..5)
            .map(|i| {
                let (client, url) = (client.clone(), proxy.url_for("ep.mkv"));
                tokio::spawn(async move {
                    let r = client.get(url).header(RANGE, format!("bytes={}-{}", i * 10, i * 10 + 9)).send().await.unwrap();
                    let status = r.status();
                    r.bytes().await.unwrap();
                    status
                })
            })
            .collect();
        for t in tasks {
            assert_eq!(t.await.unwrap(), StatusCode::PARTIAL_CONTENT);
        }
        assert!(fake.peak.load(Ordering::SeqCst) <= 2, "pico = {}", fake.peak.load(Ordering::SeqCst));
        assert_eq!(fake.calls.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn preview_lane_does_not_wait_for_the_player() {
        // Dois corpos do player abertos (lentos) ocupam as duas vagas dele.
        let fake = Fake { chunk_delay: Some(Duration::from_secs(30)), ..Fake::new() };
        let proxy = StreamProxy::start(fake.clone()).await.unwrap();
        let client = Client::new();
        let mut held = Vec::new();
        for i in 0..MAX_UPSTREAM_PER_FILE {
            let r = client.get(proxy.url_for("ep.mkv")).header(RANGE, format!("bytes={}-", i * 10)).send().await.unwrap();
            held.push(r);
        }
        // Miniatura do mesmo arquivo: vaga própria, responde sem esperar o player soltar.
        let preview = tokio::time::timeout(
            Duration::from_secs(2),
            client.get(proxy.url_for_preview("ep.mkv")).header(RANGE, "bytes=500-").send(),
        )
        .await
        .expect("miniatura ficou presa atrás do player")
        .unwrap();
        assert_eq!(preview.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(fake.last_path.lock().unwrap().as_deref(), Some("ep.mkv"), "mesmo arquivo na nuvem");
        drop(held);
    }

    #[tokio::test]
    async fn preview_lane_is_capped_too() {
        let fake = Fake { chunk_delay: Some(Duration::from_secs(30)), ..Fake::new() };
        let proxy = StreamProxy::start(fake.clone()).await.unwrap();
        let client = Client::new();
        let mut held = Vec::new();
        for i in 0..MAX_UPSTREAM_PER_PREVIEW {
            held.push(client.get(proxy.url_for_preview("ep.mkv")).header(RANGE, format!("bytes={}-", i * 10)).send().await.unwrap());
        }
        let blocked = tokio::time::timeout(
            Duration::from_millis(400),
            client.get(proxy.url_for_preview("ep.mkv")).header(RANGE, "bytes=900-").send(),
        )
        .await;
        assert!(blocked.is_err(), "a faixa de miniaturas passou do limite");
        drop(held);
    }

    #[tokio::test]
    async fn cancels_on_client_drop() {
        let fake = Fake { chunk_delay: Some(Duration::from_millis(50)), ..Fake::new() };
        let proxy = StreamProxy::start(fake.clone()).await.unwrap();
        let mut resp = Client::new().get(proxy.url_for("ep.mkv")).send().await.unwrap();
        assert!(resp.chunk().await.unwrap().is_some());
        drop(resp);
        let mut ok = false;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(25)).await;
            if fake.active.load(Ordering::SeqCst) == 0 {
                ok = true;
                break;
            }
        }
        assert!(ok, "conexão com a nuvem não foi cancelada");
    }
}
