//! Estado compartilhado pelas telas: sessão, navegação, imagens, AniList em
//! lote, acervo da temporada, downloads e avisos. As telas observam e redesenham.

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, Global, ImageFormat, Task};
use higanbana_core::{
    Core,
    anilist::{Lookup, LookupRequest, MediaType, types::MediaSummary},
    auth::Session,
    nuvem::download::DownloadProgress,
    player::store::PlayerData,
};

use crate::video::PlayTarget;

const LOOKUP_DELAY: Duration = Duration::from_millis(60);
const LOOKUP_MAX: usize = 40;
/// Espera antes de repetir buscas adiadas (fallback MAL com limite de requisições).
const LOOKUP_RETRY: Duration = Duration::from_secs(3);
const MAX_DOWNLOADS: usize = 2;

#[derive(Clone, Debug)]
pub struct User {
    pub name: String,
    pub avatar: Option<String>,
}

/// Nome de exibição do Discord guardado na sessão.
pub fn user_of(s: &Session) -> User {
    let meta = s.user.as_ref().and_then(|u| u.get("user_metadata"));
    let get = |k: &str| meta.and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(str::to_owned);
    User {
        name: get("full_name").or_else(|| get("name")).or_else(|| get("user_name")).unwrap_or_else(|| "Você".into()),
        avatar: get("avatar_url"),
    }
}

#[derive(Clone)]
pub enum Img {
    Loading,
    Ready(Arc<gpui_kit::Image>),
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlStatus {
    Queued,
    Running,
    Done,
    Error,
}

#[derive(Clone, Debug)]
pub struct Download {
    pub id: String,
    pub path: String,
    pub name: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub status: DlStatus,
    pub dest: Option<std::path::PathBuf>,
    pub error: Option<String>,
}

impl Download {
    pub fn progress(&self) -> f32 {
        self.total.filter(|t| *t > 0).map_or(0., |t| (self.downloaded as f64 / t as f64).min(1.) as f32)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Ok,
    Error,
}

/// Pedidos que a raiz atende (abrir player/leitor, trazer a janela para frente).
pub enum AppEvent {
    Play(PlayTarget),
    Read(crate::reader::ReadTarget),
    Navigated,
    /// Aviso curto para o usuário (vira notificação na janela).
    Notify(Tone, String),
}

pub struct AppState {
    pub core: Arc<Core>,
    /// `None` antes de ler a sessão salva.
    pub session_ready: bool,
    pub user: Option<User>,
    stack: Vec<String>,
    index: usize,
    images: HashMap<String, Img>,
    lookups: HashMap<(MediaType, String), Option<Option<MediaSummary>>>,
    lookup_queue: Vec<(MediaType, String)>,
    lookup_task: Option<Task<()>>,
    catalog: HashMap<i64, Option<Option<String>>>,
    pub downloads: Vec<Download>,
    next_download: u64,
    pub player_data: PlayerData,
    pub music: Option<higanbana_core::music::engine::PlayerState>,
    pub music_session: crate::music::session::MusicSession,
    pub(crate) cat: crate::catalog_state::CatalogState,
}

struct GlobalState(Entity<AppState>);
impl Global for GlobalState {}

impl EventEmitter<AppEvent> for AppState {}

/// Estado do app (registrado em `main`).
pub fn state(cx: &App) -> Entity<AppState> {
    cx.global::<GlobalState>().0.clone()
}

pub fn install(core: Arc<Core>, cx: &mut App) -> Entity<AppState> {
    let player_data = core.player().data();
    let s = cx.new(|cx| {
        let mut s = AppState {
            core,
            session_ready: false,
            user: None,
            stack: vec![String::new()],
            index: 0,
            images: HashMap::new(),
            lookups: HashMap::new(),
            lookup_queue: Vec::new(),
            lookup_task: None,
            catalog: HashMap::new(),
            downloads: Vec::new(),
            next_download: 1,
            player_data,
            music: None,
            music_session: Default::default(),
            cat: Default::default(),
        };
        s.load_session(cx);
        s
    });
    cx.set_global(GlobalState(s.clone()));
    s
}

/// Formato da imagem pelos primeiros bytes.
pub fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    Some(match bytes {
        [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
        [0xFF, 0xD8, 0xFF, ..] => ImageFormat::Jpeg,
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => ImageFormat::Webp,
        [b'G', b'I', b'F', ..] => ImageFormat::Gif,
        [b'B', b'M', ..] => ImageFormat::Bmp,
        _ => return None,
    })
}

impl AppState {
    fn load_session(&mut self, cx: &mut Context<Self>) {
        let core = self.core.clone();
        let task = self.core.spawn(async move { core.auth().session().await });
        cx.spawn(async move |this, cx| {
            let session = task.await.ok().and_then(Result::ok).flatten();
            let _ = this.update(cx, |s, cx| {
                s.user = session.as_ref().map(user_of);
                s.session_ready = true;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_session(&mut self, session: Option<&Session>, cx: &mut Context<Self>) {
        self.user = session.map(user_of);
        self.session_ready = true;
        cx.notify();
    }

    // --- Navegação ---

    pub fn path(&self) -> &str {
        &self.stack[self.index]
    }

    pub fn go(&mut self, path: impl Into<String>, cx: &mut Context<Self>) {
        let path = path.into();
        if path == self.path() {
            return;
        }
        self.stack.truncate(self.index + 1);
        self.stack.push(path);
        self.index += 1;
        cx.emit(AppEvent::Navigated);
        cx.notify();
    }

    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.stack.len()
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.can_back() {
            self.index -= 1;
            cx.emit(AppEvent::Navigated);
            cx.notify();
        }
    }

    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() {
            self.index += 1;
            cx.emit(AppEvent::Navigated);
            cx.notify();
        }
    }

    pub fn play(&mut self, target: PlayTarget, cx: &mut Context<Self>) {
        cx.emit(AppEvent::Play(target));
    }

    pub fn read(&mut self, target: crate::reader::ReadTarget, cx: &mut Context<Self>) {
        cx.emit(AppEvent::Read(target));
    }

    // --- Imagens ---

    /// Imagem de uma URL pública; começa a baixar na primeira vez.
    pub fn image(&mut self, url: &str, cx: &mut Context<Self>) -> Img {
        if let Some(i) = self.images.get(url) {
            return i.clone();
        }
        self.images.insert(url.to_owned(), Img::Loading);
        let core = self.core.clone();
        let owned = url.to_owned();
        let task = self.core.spawn(async move { core.fetch_public(&owned).await });
        let key = url.to_owned();
        cx.spawn(async move |this, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |s, cx| {
                let img = match result.as_deref().and_then(|b| Some((sniff(b)?, b))) {
                    Some((fmt, b)) => Img::Ready(Arc::new(gpui_kit::Image::from_bytes(fmt, b.to_vec()))),
                    None => Img::Failed,
                };
                s.images.insert(key, img);
                cx.notify();
            });
        })
        .detach();
        Img::Loading
    }

    /// Guarda bytes já baixados (páginas, capas da nuvem) sob uma chave.
    pub fn put_image(&mut self, key: String, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let img = match sniff(&bytes) {
            Some(fmt) => Img::Ready(Arc::new(gpui_kit::Image::from_bytes(fmt, bytes))),
            None => Img::Failed,
        };
        self.images.insert(key, img);
        cx.notify();
    }

    pub fn cached_image(&self, key: &str) -> Option<Img> {
        self.images.get(key).cloned()
    }

    pub fn mark_image_loading(&mut self, key: &str) {
        self.images.insert(key.to_owned(), Img::Loading);
    }

    pub fn fail_image(&mut self, key: &str, cx: &mut Context<Self>) {
        self.images.insert(key.to_owned(), Img::Failed);
        cx.notify();
    }

    pub fn forget_image(&mut self, key: &str) {
        self.images.remove(key);
    }

    // --- AniList em lote ---

    /// `None` = ainda buscando; `Some(None)` = sem correspondência.
    pub fn lookup(&self, name: &str, media: MediaType) -> Option<Option<MediaSummary>> {
        self.lookups.get(&(media, name.to_owned())).cloned().flatten()
    }

    /// Há buscas na fila ou prestes a sair (o rastreio espera).
    pub(crate) fn lookups_busy(&self) -> bool {
        !self.lookup_queue.is_empty() || self.lookup_task.is_some()
    }

    /// Pede a busca (junta pedidos próximos numa requisição só).
    pub fn want_lookup(&mut self, name: &str, media: MediaType, cx: &mut Context<Self>) {
        let key = (media, name.to_owned());
        if self.lookups.contains_key(&key) {
            return;
        }
        self.lookups.insert(key.clone(), None);
        self.lookup_queue.push(key);
        if self.lookup_task.is_none() {
            self.lookup_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(LOOKUP_DELAY).await;
                let _ = this.update(cx, |s, cx| s.flush_lookups(cx));
            }));
        }
    }

    fn flush_lookups(&mut self, cx: &mut Context<Self>) {
        self.lookup_task = None;
        let n = self.lookup_queue.len().min(LOOKUP_MAX);
        let batch: Vec<(MediaType, String)> = self.lookup_queue.drain(..n).collect();
        if !self.lookup_queue.is_empty() {
            self.lookup_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(LOOKUP_DELAY).await;
                let _ = this.update(cx, |s, cx| s.flush_lookups(cx));
            }));
        }
        if batch.is_empty() {
            return;
        }
        let core = self.core.clone();
        let items: Vec<LookupRequest> = batch.iter().map(|(m, n)| LookupRequest { name: n.clone(), media_type: *m }).collect();
        let task = self.core.spawn(async move { core.anilist().lookup(&items).await });
        cx.spawn(async move |this, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |s, cx| {
                match result {
                    Some(list) => {
                        let mut retry = Vec::new();
                        for (key, v) in batch.into_iter().zip(list) {
                            match v {
                                Lookup::Found(v) => {
                                    s.lookups.insert(key, Some(serde_json::from_value::<MediaSummary>(v).ok()));
                                }
                                Lookup::NotFound => {
                                    s.lookups.insert(key, Some(None));
                                }
                                // Fallback MAL adiado: continua "buscando" e volta à fila depois.
                                Lookup::Pending => retry.push(key),
                            }
                        }
                        if !retry.is_empty() {
                            cx.spawn(async move |this, cx| {
                                cx.background_executor().timer(LOOKUP_RETRY).await;
                                let _ = this.update(cx, |s, cx| {
                                    s.lookup_queue.extend(retry);
                                    if s.lookup_task.is_none() {
                                        s.flush_lookups(cx);
                                    }
                                });
                            })
                            .detach();
                        }
                    }
                    // Falha de rede/limite: libera para tentar de novo depois.
                    None => {
                        for key in batch {
                            s.lookups.remove(&key);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // --- Acervo da temporada ---

    /// `None` = não conferido; `Some(None)` = não está; `Some(Some(caminho))`.
    pub fn catalog(&self, id: i64) -> Option<Option<String>> {
        self.catalog.get(&id).cloned().flatten().or_else(|| self.core.catalog().known(id))
    }

    pub fn want_catalog(&mut self, m: &MediaSummary, cx: &mut Context<Self>) {
        if self.catalog.contains_key(&m.id) || self.core.catalog().known(m.id).is_some() {
            return;
        }
        self.catalog.insert(m.id, None);
        let core = self.core.clone();
        let media = m.clone();
        let id = m.id;
        let task = self.core.spawn(async move { core.check_catalog(&media).await });
        cx.spawn(async move |this, cx| {
            let path = task.await.ok();
            let _ = this.update(cx, |s, cx| {
                match path {
                    Some(p) => {
                        s.catalog.insert(id, Some(p));
                    }
                    None => {
                        s.catalog.remove(&id);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // --- Avisos ---

    pub fn toast(&mut self, message: impl Into<String>, tone: Tone, cx: &mut Context<Self>) {
        cx.emit(AppEvent::Notify(tone, message.into()));
    }

    // --- Downloads ---

    pub fn download(&self, path: &str) -> Option<&Download> {
        self.downloads.iter().find(|d| d.path == path)
    }

    pub fn pending_downloads(&self) -> usize {
        self.downloads.iter().filter(|d| matches!(d.status, DlStatus::Running | DlStatus::Queued)).count()
    }

    /// Progresso agregado dos downloads ativos (0–1).
    pub fn overall_progress(&self) -> f32 {
        let active: Vec<&Download> = self.downloads.iter().filter(|d| d.status == DlStatus::Running && d.total.is_some()).collect();
        let total: u64 = active.iter().filter_map(|d| d.total).sum();
        if total == 0 { 0. } else { active.iter().map(|d| d.downloaded).sum::<u64>() as f32 / total as f32 }
    }

    fn enqueue(&mut self, path: &str) {
        if let Some(d) = self.downloads.iter_mut().find(|d| d.path == path) {
            if matches!(d.status, DlStatus::Running | DlStatus::Queued) {
                return;
            }
            d.status = DlStatus::Queued;
            d.error = None;
            d.downloaded = 0;
            return;
        }
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let id = format!("dl-{}-{}", std::process::id(), self.next_download);
        self.next_download += 1;
        self.downloads.insert(0, Download { id, path: path.to_owned(), name, downloaded: 0, total: None, status: DlStatus::Queued, dest: None, error: None });
    }

    pub fn start_downloads(&mut self, paths: &[String], cx: &mut Context<Self>) {
        for p in paths {
            self.enqueue(p);
        }
        self.pump(cx);
        cx.notify();
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        let running = self.downloads.iter().filter(|d| d.status == DlStatus::Running).count();
        let queued: Vec<(String, String)> = self
            .downloads
            .iter()
            .rev()
            .filter(|d| d.status == DlStatus::Queued)
            .map(|d| (d.id.clone(), d.path.clone()))
            .take(MAX_DOWNLOADS.saturating_sub(running))
            .collect();
        for (id, path) in queued {
            if let Some(d) = self.downloads.iter_mut().find(|d| d.id == id) {
                d.status = DlStatus::Running;
                d.error = None;
                d.downloaded = 0;
            }
            let core = self.core.clone();
            let (i, p) = (id.clone(), path.clone());
            let task = self.core.spawn(async move { core.cloud().download(&i, &p, None).await });
            cx.spawn(async move |this, cx| {
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r.map_err(|e| e.to_string()));
                let _ = this.update(cx, |s, cx| {
                    let name = s.downloads.iter().find(|d| d.id == id).map(|d| d.name.clone()).unwrap_or_default();
                    match result {
                        Ok(dest) => {
                            if let Some(d) = s.downloads.iter_mut().find(|d| d.id == id) {
                                d.status = DlStatus::Done;
                                d.dest = Some(dest);
                            }
                            s.toast(format!("{name} baixado"), Tone::Ok, cx);
                        }
                        Err(e) => {
                            if let Some(d) = s.downloads.iter_mut().find(|d| d.id == id) {
                                d.status = DlStatus::Error;
                                d.error = Some(e);
                            }
                            s.toast("Um download falhou. Abra Downloads para tentar de novo.", Tone::Error, cx);
                        }
                    }
                    s.pump(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub fn on_download_progress(&mut self, p: &DownloadProgress, cx: &mut Context<Self>) {
        if let Some(d) = self.downloads.iter_mut().find(|d| d.id == p.id) {
            d.downloaded = p.downloaded;
            d.total = p.total;
            cx.notify();
        }
    }

    pub fn clear_finished(&mut self, cx: &mut Context<Self>) {
        self.downloads.retain(|d| matches!(d.status, DlStatus::Running | DlStatus::Queued));
        cx.notify();
    }

    pub fn refresh_player_data(&mut self, cx: &mut Context<Self>) {
        self.player_data = self.core.player().data();
        cx.notify();
    }
}
