//! Login pelo navegador padrão do sistema (onde o Discord já está logado).
//!
//! 1. Sobe um servidor HTTP mínimo em `127.0.0.1:<porta aleatória>`.
//! 2. Abre `auth.anitsu.moe/auth/v1/authorize?provider=discord` no navegador,
//!    com `redirect_to` apontando para esse servidor e PKCE.
//! 3. O GoTrue aceita redirecionar para loopback por IP (RFC 8252) e devolve
//!    `?code=...`, que é trocado pela sessão usando o `verifier` local.

use std::{net::Ipv4Addr, time::Duration};

use url::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use super::{
    error::{AuthError, AuthResult},
    gotrue::GoTrueClient,
    pkce::Pkce,
    session::Session,
};
use crate::events::{Event, EventFn};

const PROVIDER: &str = "discord";
const CALLBACK_PATH: &str = "/callback";
const TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_REQUEST_BYTES: usize = 16 * 1024;
/// Quanto a troca do código espera a rede voltar (o usuário voltar ao app).
const OFFLINE_WINDOW: Duration = Duration::from_secs(5 * 60);
/// Intervalo entre tentativas enquanto a rede não volta.
const OFFLINE_RETRY: Duration = Duration::from_secs(1);

/// Login em andamento: URL para reabrir o navegador e canal de cancelamento.
pub struct PendingLogin {
    pub url: String,
    pub cancel: oneshot::Sender<()>,
}

/// Prepara o servidor local e a URL de autorização.
pub async fn start(gotrue: &GoTrueClient) -> AuthResult<(TcpListener, Pkce, String)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let redirect = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let pkce = Pkce::new();
    let url = gotrue.authorize_url(PROVIDER, &redirect, &pkce.challenge);
    Ok((listener, pkce, url))
}

pub fn open_browser(url: &str) -> AuthResult<()> {
    open::that_detached(url)
        .map_err(|e| AuthError::Provider(format!("não foi possível abrir o navegador: {e}")))
}

/// Espera o retorno do navegador (ou cancelamento/timeout) e troca o código pela sessão.
pub async fn finish(
    events: &EventFn,
    gotrue: &GoTrueClient,
    listener: TcpListener,
    pkce: Pkce,
    mut cancel: oneshot::Receiver<()>,
) -> AuthResult<Session> {
    let code = tokio::select! {
        r = wait_for_code(&listener) => r?,
        _ = &mut cancel => return Err(AuthError::Cancelled),
        _ = tokio::time::sleep(TIMEOUT) => return Err(AuthError::Timeout),
    };
    drop(listener);
    events(Event::LoginReturned);
    exchange_when_online(&mut cancel, || gotrue.exchange_code(&code, &pkce.verifier)).await
}

/// Troca o código, esperando a rede se a requisição não conseguiu sair.
///
/// No Android o app fica em segundo plano enquanto o navegador está na frente, e
/// o sistema corta a internet dele (o loopback do callback continua liberado): a
/// troca falha sem chegar ao servidor até o usuário voltar ao app. Só se repete
/// esse caso; um erro que veio do servidor (código usado, recusado) volta na hora.
async fn exchange_when_online<T, F, Fut>(cancel: &mut oneshot::Receiver<()>, exchange: F) -> AuthResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = AuthResult<T>>,
{
    let deadline = tokio::time::Instant::now() + OFFLINE_WINDOW;
    loop {
        match exchange().await {
            Err(AuthError::Unreachable(_)) if tokio::time::Instant::now() < deadline => {
                tokio::select! {
                    _ = &mut *cancel => return Err(AuthError::Cancelled),
                    _ = tokio::time::sleep(OFFLINE_RETRY) => {}
                }
            }
            other => return other,
        }
    }
}

/// Aceita conexões até chegar um `GET /callback` com `code` ou `error`.
async fn wait_for_code(listener: &TcpListener) -> AuthResult<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let Some(target) = read_request_target(&mut stream).await else {
            continue;
        };
        match parse_callback(&target) {
            Callback::Code(code) => {
                respond(&mut stream, 200, &page(true, "Pronto! Você já pode fechar esta aba e voltar ao Higanbana.")).await;
                return Ok(code);
            }
            Callback::Error(msg) => {
                respond(&mut stream, 400, &page(false, &format!("O login não foi concluído: {msg}"))).await;
                return Err(AuthError::Provider(msg));
            }
            // favicon.ico, prefetch etc.
            Callback::Other => respond(&mut stream, 404, "").await,
        }
    }
}

async fn read_request_target(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST_BYTES {
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let line = String::from_utf8_lossy(&buf).lines().next()?.to_owned();
    let mut parts = line.split_whitespace();
    (parts.next()? == "GET").then_some(())?;
    parts.next().map(str::to_owned)
}

#[derive(Debug, PartialEq)]
enum Callback {
    Code(String),
    Error(String),
    Other,
}

fn parse_callback(target: &str) -> Callback {
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Callback::Other;
    };
    if url.path() != CALLBACK_PATH {
        return Callback::Other;
    }
    let get = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
    if let Some(code) = get("code").filter(|c| !c.is_empty()) {
        return Callback::Code(code);
    }
    match get("error_description").or_else(|| get("error")) {
        Some(msg) => Callback::Error(msg),
        None => Callback::Error("resposta sem código de autorização".into()),
    }
}

async fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn page(ok: bool, message: &str) -> String {
    let color = if ok { "#ff7a18" } else { "#ff6b6b" };
    let message = message.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        r#"<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><title>Higanbana</title>
<style>body{{margin:0;height:100vh;display:grid;place-items:center;background:#100e17;color:#ece8f4;font:16px/1.5 system-ui,sans-serif}}
main{{max-width:420px;padding:24px}}h1{{margin:0 0 12px;font-size:56px;letter-spacing:-.04em;color:{color}}}p{{margin:0;color:#9a93ad}}</style>
</head><body><main><h1>higanbana</h1><p>{message}</p></main></body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_code() {
        assert_eq!(parse_callback("/callback?code=abc-123"), Callback::Code("abc-123".into()));
    }

    #[test]
    fn parses_error() {
        assert_eq!(
            parse_callback("/callback?error=access_denied&error_description=The+resource+owner+denied"),
            Callback::Error("The resource owner denied".into())
        );
    }

    #[test]
    fn ignores_other_paths() {
        assert_eq!(parse_callback("/favicon.ico"), Callback::Other);
    }

    /// No Android o app em segundo plano não sai para a internet (só o loopback):
    /// a troca falha sem chegar ao servidor até o usuário voltar ao app.
    #[tokio::test(start_paused = true)]
    async fn troca_espera_a_rede_voltar() {
        let tries = std::sync::atomic::AtomicUsize::new(0);
        let (_keep, mut cancel) = oneshot::channel::<()>();
        let r = exchange_when_online(&mut cancel, || async {
            if tries.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 3 {
                Err(AuthError::Unreachable("bloqueado".into()))
            } else {
                Ok("sessão")
            }
        })
        .await;
        assert_eq!(r.unwrap(), "sessão");
        assert_eq!(tries.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn troca_nao_repete_o_que_chegou_ao_servidor() {
        let tries = std::sync::atomic::AtomicUsize::new(0);
        let (_keep, mut cancel) = oneshot::channel::<()>();
        let r: AuthResult<()> = exchange_when_online(&mut cancel, || async {
            tries.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(AuthError::Provider("código inválido".into()))
        })
        .await;
        assert!(matches!(r, Err(AuthError::Provider(_))));
        assert_eq!(tries.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn troca_desiste_no_cancelar_e_no_prazo() {
        let (tx, mut cancel) = oneshot::channel::<()>();
        tx.send(()).unwrap();
        let r: AuthResult<()> = exchange_when_online(&mut cancel, || async { Err(AuthError::Unreachable("x".into())) }).await;
        assert!(matches!(r, Err(AuthError::Cancelled)));

        let (_keep, mut cancel) = oneshot::channel::<()>();
        let r: AuthResult<()> = exchange_when_online(&mut cancel, || async { Err(AuthError::Unreachable("x".into())) }).await;
        assert!(matches!(r, Err(AuthError::Unreachable(_))));
    }

    #[tokio::test]
    async fn loopback_roundtrip() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = tokio::spawn(async move {
            let get = |path: &'static str| async move {
                let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.unwrap();
                s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).await.unwrap();
                let mut out = String::new();
                s.read_to_string(&mut out).await.unwrap();
                out
            };
            let fav = get("/favicon.ico").await;
            let ok = get("/callback?code=xyz").await;
            (fav, ok)
        });
        assert_eq!(wait_for_code(&listener).await.unwrap(), "xyz");
        let (fav, ok) = client.await.unwrap();
        assert!(fav.starts_with("HTTP/1.1 404"));
        assert!(ok.starts_with("HTTP/1.1 200") && ok.contains("fechar esta aba"));
    }
}
