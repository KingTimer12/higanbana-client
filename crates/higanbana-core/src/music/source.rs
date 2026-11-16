//! Faixa da nuvem lida em stream para a RAM, com `seek`.
//!
//! Um pedido aberto por vez (`Range: bytes=N-`). Ler um ponto que ainda não
//! chegou e está longe do que está chegando abre outro pedido a partir dali;
//! o que já chegou fica guardado. Nada vai para o disco.

use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use reqwest::{Method, header::RANGE};
use tokio::runtime::Handle;

use crate::cloud::Cloud;

/// Se o ponto pedido está até essa distância à frente do que está chegando, espera em vez de reabrir.
const AHEAD_WAIT: u64 = 512 * 1024;
/// Tentativas ao receber 429 ou cair a conexão.
const RETRIES: u32 = 5;
/// Sem nenhum byte por esse tempo, a conexão é tratada como caída.
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// De onde vêm os bytes: produção = nuvem, testes = memória.
pub trait Fetcher: Send + Sync + 'static {
    /// Abre um stream a partir de `offset` e entrega cada pedaço a `sink` até o
    /// fim ou até `sink` devolver `false` (cancelado).
    fn stream(&self, offset: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String>;
}

#[derive(Default)]
struct Buf {
    data: Vec<u8>,
    /// Intervalos recebidos `[início, fim)`, ordenados e sem sobreposição.
    have: Vec<(u64, u64)>,
    /// Geração do pedido atual; pedidos antigos param ao ver outra geração.
    generation: u64,
    start: u64,
    front: u64,
    done: bool,
    error: Option<String>,
    closed: bool,
}

impl Buf {
    /// Fim do trecho contínuo recebido que contém `pos`.
    fn contiguous_end(&self, pos: u64) -> Option<u64> {
        self.have.iter().find(|(s, e)| *s <= pos && pos < *e).map(|(_, e)| *e)
    }

    fn add(&mut self, s: u64, e: u64) {
        self.have.push((s, e));
        self.have.sort_unstable();
        // Junta no lugar (sem alocar): roda a cada pedaço que chega da rede.
        self.have.dedup_by(|next, last| {
            let overlaps = next.0 <= last.1;
            if overlaps {
                last.1 = last.1.max(next.1);
            }
            overlaps
        });
    }
}

struct Shared {
    buf: Mutex<Buf>,
    cv: Condvar,
    fetcher: Arc<dyn Fetcher>,
    len: u64,
}

/// Abre um pedido novo a partir de `offset` (o anterior é abandonado).
fn start(shared: &Arc<Shared>, b: &mut Buf, offset: u64) {
    b.generation += 1;
    b.start = offset;
    b.front = offset;
    b.done = false;
    b.error = None;
    let (me, generation) = (shared.clone(), b.generation);
    std::thread::spawn(move || {
        let mut at = offset;
        let r = me.fetcher.stream(offset, &mut |bytes| {
            let mut b = me.buf.lock().unwrap();
            if b.generation != generation || b.closed {
                return false;
            }
            // Pedaço vazio = só "ainda querem?" (o fetcher pergunta antes de tentar de novo).
            if bytes.is_empty() {
                return true;
            }
            if b.data.is_empty() {
                b.data = vec![0; me.len as usize];
            }
            let end = (at + bytes.len() as u64).min(me.len);
            let n = (end - at) as usize;
            b.data[at as usize..end as usize].copy_from_slice(&bytes[..n]);
            b.add(at, end);
            at = end;
            b.front = end;
            me.cv.notify_all();
            true
        });
        let mut b = me.buf.lock().unwrap();
        if b.generation == generation {
            b.done = true;
            b.error = r.err();
            me.cv.notify_all();
        }
    });
}

/// Faixa da nuvem como `Read + Seek` (e `MediaSource` do symphonia).
pub struct RangeSource {
    shared: Arc<Shared>,
    pos: u64,
}

impl RangeSource {
    pub fn new(fetcher: Arc<dyn Fetcher>, len: u64) -> Self {
        let shared = Arc::new(Shared { buf: Mutex::default(), cv: Condvar::new(), fetcher, len });
        {
            let mut b = shared.buf.lock().unwrap();
            start(&shared, &mut b, 0);
        }
        Self { shared, pos: 0 }
    }

    /// Intervalos já recebidos.
    #[cfg(test)]
    pub fn received(&self) -> Vec<(u64, u64)> {
        self.shared.buf.lock().unwrap().have.clone()
    }

    /// Outro leitor da mesma faixa (mesmo buffer), para medir sem atrapalhar a posição.
    pub fn handle(&self) -> BufferHandle {
        BufferHandle(self.shared.clone())
    }
}

/// Acesso ao progresso do download de uma faixa sem ser dono dela.
#[derive(Clone)]
pub struct BufferHandle(Arc<Shared>);

impl BufferHandle {
    /// Abandona a faixa: o download para e quem está lendo recebe erro.
    pub fn cancel(&self) {
        self.0.buf.lock().unwrap().closed = true;
        self.0.cv.notify_all();
    }

    pub fn buffered(&self) -> f64 {
        let b = self.0.buf.lock().unwrap();
        let got: u64 = b.have.iter().map(|(s, e)| e - s).sum();
        if self.0.len == 0 { 1.0 } else { got as f64 / self.0.len as f64 }
    }
}

impl Drop for RangeSource {
    fn drop(&mut self) {
        self.shared.buf.lock().unwrap().closed = true;
        self.shared.cv.notify_all();
    }
}

impl Read for RangeSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let sh = &self.shared;
        if self.pos >= sh.len || out.is_empty() {
            return Ok(0);
        }
        let mut b = sh.buf.lock().unwrap();
        loop {
            if b.closed {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "faixa abandonada"));
            }
            if let Some(end) = b.contiguous_end(self.pos) {
                let n = ((end - self.pos) as usize).min(out.len());
                out[..n].copy_from_slice(&b.data[self.pos as usize..self.pos as usize + n]);
                self.pos += n as u64;
                return Ok(n);
            }
            if let Some(e) = &b.error {
                return Err(io::Error::other(e.clone()));
            }
            let reachable = !b.done && self.pos >= b.start && self.pos <= b.front + AHEAD_WAIT;
            if !reachable {
                start(sh, &mut b, self.pos);
            }
            b = sh.cv.wait(b).unwrap();
        }
    }
}

impl Seek for RangeSource {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let len = self.shared.len as i128;
        let p = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(d) => len + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if p < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek antes do início"));
        }
        self.pos = p as u64;
        Ok(self.pos)
    }
}

impl symphonia::core::io::MediaSource for RangeSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.shared.len)
    }
}

/// Bytes da faixa na nuvem (`/api/preview` com Range), com a sessão do Rust.
pub struct CloudFetcher {
    pub rt: Handle,
    pub cloud: Arc<Cloud>,
    pub path: String,
}

/// Segundos do `Retry-After` (padrão 5).
pub fn retry_after(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(5)
}

impl Fetcher for CloudFetcher {
    fn stream(&self, offset: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
        self.rt.block_on(async {
            let (apis, auth) = (&self.cloud.apis, &self.cloud.auth);
            let mut at = offset;
            let mut tries = 0;
            // Espera em passos curtos, desistindo se a faixa foi abandonada.
            let sleep = async |secs: u64, sink: &mut dyn FnMut(&[u8]) -> bool| {
                for _ in 0..secs * 4 {
                    if !sink(&[]) {
                        return false;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                true
            };
            loop {
                if !sink(&[]) {
                    return Ok(());
                }
                let resp = apis
                    .nuvem
                    .send_raw(auth, Method::GET, "preview", |r| {
                        r.query(&[("path", self.path.as_str())]).header(RANGE, format!("bytes={at}-"))
                    })
                    .await;
                let mut resp = match resp {
                    Ok(r) => r,
                    Err(_) if tries < RETRIES => {
                        tries += 1;
                        if !sleep(2, sink).await {
                            return Ok(());
                        }
                        continue;
                    }
                    Err(e) => return Err(e.to_string()),
                };
                let status = resp.status().as_u16();
                if status == 429 && tries < RETRIES {
                    tries += 1;
                    if !sleep(retry_after(&resp), sink).await {
                        return Ok(());
                    }
                    continue;
                }
                if status != 206 && status != 200 {
                    return Err(format!("a nuvem respondeu {status}"));
                }
                // Servidor que ignora Range manda desde o começo: pula até `at`.
                let mut skip = if status == 200 { at } else { 0 };
                loop {
                    // Conexão parada sem erro também conta como queda.
                    let next = match tokio::time::timeout(CHUNK_TIMEOUT, resp.chunk()).await {
                        Ok(r) => r.map_err(|e| e.to_string()),
                        Err(_) => Err("a nuvem parou de responder".to_string()),
                    };
                    match next {
                        Ok(Some(c)) => {
                            let c = if skip > 0 {
                                let n = skip.min(c.len() as u64) as usize;
                                skip -= n as u64;
                                c.slice(n..)
                            } else {
                                c
                            };
                            if c.is_empty() {
                                continue;
                            }
                            at += c.len() as u64;
                            if !sink(&c) {
                                return Ok(());
                            }
                        }
                        Ok(None) => return Ok(()),
                        // Conexão caiu no meio: retoma de onde parou.
                        Err(_) if tries < RETRIES => {
                            tries += 1;
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct MemFetcher {
        data: Vec<u8>,
        chunk: usize,
        delay: Option<Duration>,
        calls: Mutex<Vec<u64>>,
        fail: bool,
    }

    impl MemFetcher {
        fn new(data: Vec<u8>) -> Self {
            Self { data, chunk: 16 * 1024, delay: None, calls: Mutex::default(), fail: false }
        }
    }

    impl Fetcher for MemFetcher {
        fn stream(&self, offset: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
            self.calls.lock().unwrap().push(offset);
            if self.fail {
                return Err("rede caiu".into());
            }
            for c in self.data[offset as usize..].chunks(self.chunk) {
                if let Some(d) = self.delay {
                    std::thread::sleep(d);
                }
                if !sink(c) {
                    return Ok(());
                }
            }
            Ok(())
        }
    }

    fn data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 % 251) as u8).collect()
    }

    #[test]
    fn reads_whole_file_in_order() {
        let d = data(300_000);
        let f = Arc::new(MemFetcher::new(d.clone()));
        let mut s = RangeSource::new(f.clone(), d.len() as u64);
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        assert_eq!(out, d);
        assert_eq!(*f.calls.lock().unwrap(), vec![0]);
    }

    #[test]
    fn seek_past_received_restarts_fetch_at_offset() {
        let d = data(4 * 1024 * 1024);
        let mut m = MemFetcher::new(d.clone());
        m.delay = Some(Duration::from_millis(1));
        let f = Arc::new(m);
        let mut s = RangeSource::new(f.clone(), d.len() as u64);
        let at = 3 * 1024 * 1024u64;
        s.seek(SeekFrom::Start(at)).unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, &d[at as usize..at as usize + 4]);
        assert!(f.calls.lock().unwrap().contains(&at));
    }

    #[test]
    fn seek_back_into_received_does_not_refetch() {
        let d = data(100_000);
        let f = Arc::new(MemFetcher::new(d.clone()));
        let mut s = RangeSource::new(f.clone(), d.len() as u64);
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        s.seek(SeekFrom::Start(10)).unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, &d[10..14]);
        assert_eq!(f.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn fetch_error_surfaces_as_io_error() {
        let mut m = MemFetcher::new(data(1000));
        m.fail = true;
        let mut s = RangeSource::new(Arc::new(m), 1000);
        let mut buf = [0u8; 4];
        assert!(s.read(&mut buf).is_err());
    }

    #[test]
    fn reports_received_ranges() {
        let d = data(50_000);
        let f = Arc::new(MemFetcher::new(d.clone()));
        let mut s = RangeSource::new(f, d.len() as u64);
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        assert_eq!(s.received(), vec![(0, 50_000)]);
    }
}
