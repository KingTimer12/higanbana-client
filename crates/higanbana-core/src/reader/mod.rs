//! Leitor de mangá: abre `.cbz`/`.zip` da nuvem por pedaços, sem gravar nada no disco.
//!
//! - [`Reader::open`] lê só a cauda e o índice do zip e devolve as imagens (ou os
//!   `.cbz` guardados dentro dele, nas coletâneas como "Caps. 1-100.zip");
//! - [`Reader::page`] busca só os bytes de uma imagem com `Range`.
//!
//! A sessão fica no núcleo: a interface nunca vê cookie nem token.

pub mod api;
pub mod logic;
pub mod rar;
pub mod store;
#[cfg(feature = "rar")]
pub mod unpack;
pub mod zip;

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use reqwest::{Method, header::RANGE};
use tokio::sync::Semaphore;

pub use api::{OpenResult, PageInfo, contents_of};
use crate::cloud::Cloud;
use zip::{Entry, Located};

/// Zips abertos recentemente (inclui os `.cbz` internos de coletâneas).
const CACHE_ARCHIVES: usize = 12;
/// Páginas baixando ao mesmo tempo.
const MAX_PAGE_FETCHES: usize = 4;
/// Rars descompactados guardados na memória (um capítulo tem uns 10 MB).
const CACHE_UNPACKED: usize = 3;

type Unpacked = Arc<HashMap<String, Vec<u8>>>;

pub struct Reader {
    cloud: Arc<Cloud>,
    pub store: store::ReaderStore,
    archives: Mutex<VecDeque<(String, Arc<Archive>)>>,
    fetches: Semaphore,
    unpacked: Mutex<VecDeque<(String, Unpacked)>>,
    /// Um rar descompactando por vez (as páginas dele esperam em vez de baixar de novo).
    unpacking: tokio::sync::Mutex<()>,
}

/// Um zip aberto: a janela de bytes que ele ocupa no arquivo da nuvem e suas entradas.
/// A janela é o arquivo todo ou um `.cbz` guardado sem compressão dentro de um `.zip`.
pub struct Archive {
    pub base: u64,
    pub len: u64,
    pub entries: Vec<Entry>,
}

impl Reader {
    pub fn new(cloud: Arc<Cloud>) -> Self {
        #[cfg(feature = "rar")]
        unpack::clean_leftovers();
        Self {
            store: store::ReaderStore::open(cloud.paths.data.join("reader.json")),
            cloud,
            archives: Mutex::default(),
            fetches: Semaphore::new(MAX_PAGE_FETCHES),
            unpacked: Mutex::default(),
            unpacking: tokio::sync::Mutex::new(()),
        }
    }
}

/// Bytes `[start, end]` (inclusivo) de um arquivo da nuvem. Com a cópia baixada
/// para ver sem internet, lê dela e nem vai à rede.
pub async fn fetch_range(cloud: &Cloud, path: &str, start: u64, end: u64) -> Result<Vec<u8>, String> {
    if let Some(local) = crate::offline::copy_of(&cloud.paths.data, path) {
        return read_local(local, start, end).await;
    }
    let resp = cloud
        .apis
        .nuvem
        .send_raw(&cloud.auth, Method::GET, "preview", |r| {
            r.query(&[("path", path)]).header(RANGE, format!("bytes={start}-{end}"))
        })
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    if status != 206 && status != 200 {
        return Err(format!("a nuvem respondeu {status}"));
    }
    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    // Servidor que ignora `Range` manda o arquivo todo: recorta.
    if status == 200 {
        let s = (start as usize).min(body.len());
        let e = (end as usize + 1).min(body.len());
        return Ok(body[s..e].to_vec());
    }
    // Sem cópia quando o `Bytes` é o único dono do buffer (o caso de `resp.bytes()`).
    Ok(Vec::from(body))
}

async fn read_local(file: std::path::PathBuf, start: u64, end: u64) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::fs::File::open(&file)?;
        let len = f.metadata()?.len();
        let end = end.min(len.saturating_sub(1));
        if start > end {
            return Ok(Vec::new());
        }
        f.seek(SeekFrom::Start(start))?;
        let mut buf = vec![0; (end - start + 1) as usize];
        f.read_exact(&mut buf)?;
        Ok::<_, std::io::Error>(buf)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("cópia local ilegível: {e}"))
}

fn cache_key(path: &str, inner: Option<&str>) -> String {
    match inner {
        Some(i) => format!("{path}\u{0}{i}"),
        None => path.to_owned(),
    }
}

/// `.rar`/`.cbr` pelo nome; o resto é tratado como zip.
pub fn is_rar(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".rar") || n.ends_with(".cbr")
}

/// Índice de um rar que ocupa `[base, base + len)`: um cabeçalho por vez, pulando os dados.
async fn read_rar_index(cloud: &Cloud, path: &str, base: u64, len: u64) -> Result<Vec<Entry>, String> {
    let get = |s: u64, n: u64| fetch_range(cloud, path, base + s, base + (s + n).min(len) - 1);
    let head = get(0, 16).await?;
    let (version, mut at) = rar::detect(&head).ok_or_else(|| rar::RarError::NotRar.to_string())?;
    let mut out = Vec::new();
    while at < len {
        let mut buf = get(at, rar::HEADER_GUESS).await?;
        let parsed = loop {
            match rar::parse_block(version, &buf, at).map_err(|e| e.to_string())? {
                Ok(p) => break p,
                Err(need) if (buf.len() as u64) < (len - at) && need > buf.len() => buf = get(at, need as u64).await?,
                Err(_) => return Err(rar::RarError::Corrupt.to_string()),
            }
        };
        match parsed.block {
            rar::Block::File(e) => out.push(e),
            rar::Block::End => break,
            rar::Block::Other => {}
        }
        if parsed.next <= at {
            return Err(rar::RarError::Corrupt.to_string());
        }
        at = parsed.next;
    }
    Ok(out)
}

/// Índice de um zip que ocupa `[base, base + len)` do arquivo da nuvem.
async fn read_index(cloud: &Cloud, path: &str, base: u64, len: u64) -> Result<Vec<Entry>, String> {
    if len == 0 {
        return Err(zip::ZipError::NotZip.to_string());
    }
    let tail_start = len - zip::tail_len(len);
    let tail = fetch_range(cloud, path, base + tail_start, base + len - 1).await?;
    let loc = match zip::locate_cd(&tail, tail_start).map_err(|e| e.to_string())? {
        Located::Found(loc) => loc,
        Located::Zip64At(at) => {
            let rec = fetch_range(cloud, path, base + at, base + at + 55).await?;
            zip::parse_zip64_eocd(&rec).map_err(|e| e.to_string())?
        }
    };
    if loc.offset + loc.size > len {
        return Err(zip::ZipError::Corrupt.to_string());
    }
    // O índice costuma já ter vindo na cauda.
    let cd = if loc.offset >= tail_start {
        let s = (loc.offset - tail_start) as usize;
        tail[s..s + loc.size as usize].to_vec()
    } else if loc.size == 0 {
        Vec::new()
    } else {
        fetch_range(cloud, path, base + loc.offset, base + loc.offset + loc.size - 1).await?
    };
    zip::parse_cd(&cd).map_err(|e| e.to_string())
}

impl Reader {
/// Zip aberto (do cache ou lido da nuvem). Com `inner`, o `.cbz` guardado dentro do zip.
pub async fn archive(&self, path: &str, size: u64, inner: Option<&str>) -> Result<Arc<Archive>, String> {
    let key = cache_key(path, inner);
    let state = self;
    let cloud = &*self.cloud;
    if let Some(hit) = state.archives.lock().unwrap().iter().find(|(k, _)| *k == key) {
        return Ok(hit.1.clone());
    }
    let (base, len) = match inner {
        None => (0, size),
        Some(name) => {
            let outer = Box::pin(self.archive(path, size, None)).await?;
            let e = outer.entries.iter().find(|e| e.name == name).ok_or("capítulo não existe neste arquivo")?;
            if e.method != 0 {
                return Err("este capítulo está compactado dentro do arquivo; baixe para ler".into());
            }
            match e.data_offset {
                Some(o) => (outer.base + o, e.compressed_size),
                None => {
                    let at = outer.base + e.local_offset;
                    let header = fetch_range(cloud, path, at, at + 29).await?;
                    (at + zip::local_data_offset(&header).map_err(|e| e.to_string())?, e.compressed_size)
                }
            }
        }
    };
    let entries = if is_rar(inner.unwrap_or(path)) {
        read_rar_index(cloud, path, base, len).await?
    } else {
        read_index(cloud, path, base, len).await?
    };
    let archive = Arc::new(Archive { base, len, entries });

    let mut cache = state.archives.lock().unwrap();
    cache.retain(|(k, _)| *k != key);
    cache.push_front((key, archive.clone()));
    cache.truncate(CACHE_ARCHIVES);
    Ok(archive)
}

/// Rar com imagens compactadas: baixa o rar inteiro (o capítulo) e descompacta tudo.
async fn unpacked(&self, page: &PageRef, arc: &Archive) -> Result<Unpacked, String> {
    let key = cache_key(&page.path, page.inner.as_deref());
    let state = self;
    let find = || state.unpacked.lock().unwrap().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone());
    if let Some(hit) = find() {
        return Ok(hit);
    }
    let _one = state.unpacking.lock().await;
    if let Some(hit) = find() {
        return Ok(hit);
    }
    let data = fetch_range(&self.cloud, &page.path, arc.base, arc.base + arc.len - 1).await?;
    let files = tokio::task::spawn_blocking(move || unpack_rar(&data)).await.map_err(|e| e.to_string())??;
    let files = Arc::new(files);
    let mut cache = state.unpacked.lock().unwrap();
    cache.push_front((key, files.clone()));
    cache.truncate(CACHE_UNPACKED);
    Ok(files)
}

/// Bytes e nome da imagem `index` (posição no índice do zip).
pub async fn page(&self, page: &PageRef) -> Result<(Vec<u8>, String), String> {
    let arc = self.archive(&page.path, page.size, page.inner.as_deref()).await?;
    let entry = arc.entries.get(page.index).ok_or("página não existe")?;
    let state = self;
    let cloud = &*self.cloud;
    let _permit = state.fetches.acquire().await.map_err(|e| e.to_string())?;
    // RAR: o cabeçalho já disse onde os dados estão; só dá para ler se não estiverem compactados.
    if let Some(o) = entry.data_offset {
        if entry.method != 0 {
            let files = self.unpacked(page, &arc).await?;
            let data = files.get(&entry.name).ok_or("página não encontrada no rar")?;
            return Ok((data.clone(), entry.name.clone()));
        }
        if entry.compressed_size == 0 {
            return Ok((Vec::new(), entry.name.clone()));
        }
        let data = fetch_range(cloud, &page.path, arc.base + o, arc.base + o + entry.compressed_size - 1).await?;
        return Ok((data, entry.name.clone()));
    }
    let (start, end) = entry.fetch_range();
    let mut local = fetch_range(cloud, &page.path, arc.base + start, arc.base + end.min(arc.len - 1)).await?;
    loop {
        // Deflate é trabalho de CPU: sai das threads do runtime quando os dados já chegaram todos.
        let complete = entry.method == 8
            && zip::local_data_offset(&local).is_ok_and(|o| o + entry.compressed_size <= local.len() as u64);
        if complete {
            let e = entry.clone();
            let r = tokio::task::spawn_blocking(move || zip::extract(&e, &local)).await.map_err(|e| e.to_string())?;
            return match r.map_err(|e| e.to_string())? {
                Ok(data) => Ok((data, entry.name.clone())),
                Err(_) => Err(zip::ZipError::Corrupt.to_string()),
            };
        }
        match zip::extract(entry, &local).map_err(|e| e.to_string())? {
            Ok(data) => return Ok((data, entry.name.clone())),
            Err(need) if (start + need) <= arc.len && need as usize > local.len() => {
                let from = arc.base + start + local.len() as u64;
                let more = fetch_range(cloud, &page.path, from, arc.base + start + need - 1).await?;
                local.extend(more);
            }
            Err(_) => return Err(zip::ZipError::Corrupt.to_string()),
        }
    }
}
}

#[cfg(feature = "rar")]
fn unpack_rar(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    unpack::unpack(data)
}

/// Sem a `unrar` (o app mobile), só os rars guardados sem compressão abrem.
#[cfg(not(feature = "rar"))]
fn unpack_rar(_data: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    Err("Este .rar está compactado e não abre neste aparelho.".into())
}

pub fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

/// Uma página: arquivo da nuvem, `.cbz` interno opcional e posição no índice.
#[derive(Debug, PartialEq)]
pub struct PageRef {
    pub size: u64,
    pub index: usize,
    /// `.cbz` dentro do zip (coletâneas), se houver.
    pub inner: Option<String>,
    pub path: String,
}

/// `"<tamanho>/<índice>/<inner codificado>/<caminho>"`, tudo codificado mais uma vez
/// (formato das URLs `manga://` do frontend web).
pub fn parse_page_uri(uri_path: &str) -> Option<PageRef> {
    let decoded = urlencoding::decode(uri_path.trim_start_matches('/')).ok()?;
    let mut parts = decoded.splitn(4, '/');
    let size = parts.next()?.parse().ok()?;
    let index = parts.next()?.parse().ok()?;
    let inner = urlencoding::decode(parts.next()?).ok()?.into_owned();
    let path = parts.next()?.to_owned();
    (!path.is_empty()).then_some(PageRef { size, index, inner: (!inner.is_empty()).then_some(inner), path })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Igual ao frontend: `inner` codificado, depois o `convertFileSrc` codifica tudo.
    fn uri(size: u64, index: usize, inner: &str, path: &str) -> String {
        let raw = format!("{size}/{index}/{}/{path}", urlencoding::encode(inner));
        format!("/{}", urlencoding::encode(&raw))
    }

    #[test]
    fn page_uri_roundtrip() {
        let path = "Mangás/Séries Mangás/Letra C/Chainsaw Man/Cap 01 [x].cbz";
        assert_eq!(
            parse_page_uri(&uri(1234, 5, "", path)),
            Some(PageRef { size: 1234, index: 5, inner: None, path: path.into() })
        );
    }

    #[test]
    fn page_uri_with_inner_archive() {
        let path = "Mangás/Séries Mangás/Letra B/Beastars (2016)/Caps. 1-100.zip";
        assert_eq!(
            parse_page_uri(&uri(462_000_000, 3, "Capitulo 10/a b.cbz", path)),
            Some(PageRef { size: 462_000_000, index: 3, inner: Some("Capitulo 10/a b.cbz".into()), path: path.into() })
        );
    }

    #[test]
    fn page_uri_rejects_garbage() {
        assert_eq!(parse_page_uri("/abc/1//x"), None);
        assert_eq!(parse_page_uri("/10/1//"), None);
        assert_eq!(parse_page_uri("/"), None);
    }

    #[test]
    fn mime_by_extension() {
        assert_eq!(mime_for("001.JPG"), "image/jpeg");
        assert_eq!(mime_for("a/b.webp"), "image/webp");
        assert_eq!(mime_for("x"), "application/octet-stream");
    }
}
