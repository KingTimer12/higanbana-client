//! Estado compartilhado pelas telas: sessão, navegação, imagens, AniList em
//! lote, acervo da temporada, downloads e avisos. As telas observam e redesenham.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, Global, Task};
use higanbana_core::{
    Core,
    anilist::{Lookup, LookupRequest, MediaType, Priority, types::MediaSummary},
    auth::Session,
    nuvem::download::DownloadProgress,
    player::store::PlayerData,
    ratings::{self, RatingSource},
    settings::EpisodeLayout,
    tmdb::{WorkEpisodes, matching::WorkQuery},
};

use rustc_hash::FxHashMap;

use crate::{
    image_cache::{decode, ImageCache, IMAGE_BUDGET, LANDSCAPE_MAX, PAGE_MAX, PORTRAIT_MAX},
    video::PlayTarget,
};

const LOOKUP_DELAY: Duration = Duration::from_millis(60);
/// Nomes por lote do que a pessoa está vendo (duas requisições de 18 no AniList).
const LOOKUP_MAX: usize = 36;
/// Nomes por lote do rastreio (uma requisição).
const LOOKUP_MAX_BACKGROUND: usize = 18;
/// Espera antes de repetir buscas adiadas (fallback MAL com limite de requisições); dobra a cada vez sem progresso.
const LOOKUP_RETRY: Duration = Duration::from_secs(3);
/// Tentativas seguidas sem progresso antes de desistir de um nome adiado.
const LOOKUP_RETRY_MAX: u32 = 5;
/// Pausa mínima depois de um lote que falhou (rede ou limite do AniList).
const LOOKUP_COOLDOWN: Duration = Duration::from_secs(30);
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
    Ready(Arc<gpui_kit::RenderImage>),
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
    /// Para cancelar a tarefa no núcleo enquanto roda.
    abort: Option<tokio::task::AbortHandle>,
}

impl Download {
    pub fn progress(&self) -> f32 {
        self.total.filter(|t| *t > 0).map_or(0., |t| (self.downloaded as f64 / t as f64).min(1.) as f32)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
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

/// `None` = ainda buscando; `Some(None)` = sem correspondência.
type Answer = Option<Option<Arc<MediaSummary>>>;

/// Respostas do AniList, um mapa por tipo de mídia: a consulta por `&str` não
/// precisa montar uma chave `(MediaType, String)` a cada card desenhado.
#[derive(Default)]
struct Lookups {
    anime: FxHashMap<String, Answer>,
    manga: FxHashMap<String, Answer>,
}

impl Lookups {
    fn of(&self, media: MediaType) -> &FxHashMap<String, Answer> {
        match media {
            MediaType::Anime => &self.anime,
            MediaType::Manga => &self.manga,
        }
    }

    fn of_mut(&mut self, media: MediaType) -> &mut FxHashMap<String, Answer> {
        match media {
            MediaType::Anime => &mut self.anime,
            MediaType::Manga => &mut self.manga,
        }
    }

    fn get(&self, media: MediaType, name: &str) -> Option<&Answer> {
        self.of(media).get(name)
    }

    fn contains(&self, media: MediaType, name: &str) -> bool {
        self.of(media).contains_key(name)
    }

    fn insert(&mut self, (media, name): (MediaType, String), answer: Answer) {
        self.of_mut(media).insert(name, answer);
    }

    fn remove(&mut self, (media, name): &(MediaType, String)) {
        self.of_mut(*media).remove(name);
    }
}

pub struct AppState {
    pub core: Arc<Core>,
    /// `None` antes de ler a sessão salva.
    pub session_ready: bool,
    pub user: Option<User>,
    stack: Vec<String>,
    index: usize,
    /// A última navegação voltou no histórico (a página nova entra pela esquerda).
    went_back: bool,
    images: ImageCache<Img>,
    lookups: Lookups,
    /// Muda a cada lote de respostas do AniList (nomes resolvidos).
    lookups_gen: u64,
    /// Fila do que a pessoa está vendo (cards, título, franquia, busca).
    lookup_queue: Vec<(MediaType, String)>,
    /// Fila do rastreio: só sai com `lookup_queue` vazia.
    lookup_background: Vec<(MediaType, String)>,
    lookup_task: Option<Task<()>>,
    /// Lotes enviados ao AniList ainda sem resposta.
    lookups_inflight: usize,
    /// Um lote da tela falhou: nada sai até aqui.
    lookups_cooldown_until: Option<Instant>,
    /// Um lote do rastreio falhou: só a faixa dele espera até aqui.
    background_cooldown_until: Option<Instant>,
    /// Quando sai a próxima saída agendada (`lookup_task`).
    lookup_due: Option<Instant>,
    /// Nomes adiados (`Pending`): tentativas seguidas sem progresso.
    lookup_retries: HashMap<(MediaType, String), u32>,
    /// Capa do fanart.tv por id de mídia: `None` = buscando; `Some(None)` = usar a do AniList.
    covers: HashMap<i64, Option<Option<PathBuf>>>,
    /// Ids cuja busca do fanart falhou (rede): não pede de novo até a próxima navegação.
    covers_failed: HashSet<i64>,
    /// Chave `file:` de cada capa local → id da mídia (para voltar ao AniList se o arquivo estragar).
    cover_files: HashMap<String, i64>,
    /// Episódios do TMDB por obra (`tmdb::work_key`): `None` = buscando; `Some(None)` = sem dados.
    episodes: HashMap<String, Option<Option<Arc<WorkEpisodes>>>>,
    /// Obras cuja última busca no TMDB falhou (rede, 429): a tela mostra sem dados,
    /// mas a chave sai de `episodes` para a próxima visita tentar de novo.
    episodes_failed: HashSet<String>,
    pub downloads: Vec<Download>,
    next_download: u64,
    pub player_data: PlayerData,
    /// Player de músicas em entidade própria: os tiques de posição não redesenham o app.
    pub music: Entity<crate::music::session::MusicSession>,
    pub(crate) cat: crate::catalog_state::CatalogState,
    pub(crate) update: crate::updater::UpdateState,
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
        let music = cx.new(|_| crate::music::session::MusicSession::new(core.clone()));
        let mut s = AppState {
            core,
            session_ready: false,
            user: None,
            stack: vec![String::new()],
            index: 0,
            went_back: false,
            images: ImageCache::new(IMAGE_BUDGET),
            lookups: Lookups::default(),
            lookups_gen: 0,
            lookup_queue: Vec::new(),
            lookup_background: Vec::new(),
            lookup_task: None,
            lookups_inflight: 0,
            lookups_cooldown_until: None,
            background_cooldown_until: None,
            lookup_due: None,
            lookup_retries: HashMap::new(),
            covers: HashMap::new(),
            covers_failed: HashSet::new(),
            cover_files: HashMap::new(),
            episodes: HashMap::new(),
            episodes_failed: HashSet::new(),
            downloads: Vec::new(),
            next_download: 1,
            player_data,
            music,
            cat: Default::default(),
            update: Default::default(),
        };
        s.load_session(cx);
        s.check_update(cx);
        s
    });
    cx.set_global(GlobalState(s.clone()));
    s
}

/// Solta o que o gpui guarda por conta própria das imagens descartadas do cache:
/// o decodificado no cache de assets e a textura no atlas de sprites. Só soltar o
/// `Arc<Image>` não basta, o gpui mantém os dois até pedirmos. Adiado para depois
/// do quadro atual porque `image()` roda durante o desenho, com a janela emprestada.
fn release_images(evicted: Vec<Img>, cx: &mut Context<AppState>) {
    let images: Vec<_> = evicted
        .into_iter()
        .filter_map(|i| match i {
            Img::Ready(image) => Some(image),
            _ => None,
        })
        .collect();
    if images.is_empty() {
        return;
    }
    cx.spawn(async move |_, cx| {
        let _ = cx.update(|cx| {
            for image in images {
                cx.drop_image(image, None);
            }
        });
    })
    .detach();
}

/// Chave de imagem de um arquivo local.
fn file_key(path: &std::path::Path) -> String {
    format!("file:{}", path.display())
}

/// Caminho de uma chave `file:`; `None` para URLs.
fn file_key_path(key: &str) -> Option<PathBuf> {
    key.strip_prefix("file:").map(PathBuf::from)
}

/// Qual faixa da fila sai agora e quantos nomes: o que a pessoa está vendo
/// primeiro; o rastreio só quando não há mais nada dela esperando.
pub(crate) fn next_lane(interactive: usize, background: usize) -> Option<(Priority, usize)> {
    if interactive > 0 {
        Some((Priority::Interactive, interactive.min(LOOKUP_MAX)))
    } else if background > 0 {
        Some((Priority::Background, background.min(LOOKUP_MAX_BACKGROUND)))
    } else {
        None
    }
}

/// Fim da pausa depois de um lote que falhou: a nossa espera mínima ou a pedida pelo AniList, a mais longa.
pub(crate) fn cooldown_end(now: Instant, anilist_paused: Option<Instant>) -> Instant {
    (now + LOOKUP_COOLDOWN).max(anilist_paused.unwrap_or(now))
}

/// Nome que voltou adiado (`Pending`): `(tentativas sem progresso, espera)` da próxima vez,
/// ou `None` para desistir. Se o lote resolveu algum nome já adiado (fallback MAL andando
/// aos poucos), a conta recomeça; senão a espera dobra (3, 6, 12, 24, 48 s).
pub(crate) fn next_retry(strikes: Option<u32>, progress: bool) -> Option<(u32, Duration)> {
    let n = if progress { 0 } else { strikes.map_or(0, |n| n + 1) };
    (n < LOOKUP_RETRY_MAX).then(|| (n, LOOKUP_RETRY * 2u32.pow(n)))
}

/// Até quando uma faixa espera: a pausa de falha da tela (`global`) e o 429 do cliente
/// valem para as duas; a de falha do rastreio (`background`) só para a dele.
pub(crate) fn lane_paused_until(
    priority: Priority,
    now: Instant,
    global: Option<Instant>,
    background: Option<Instant>,
    client: Option<Instant>,
) -> Option<Instant> {
    let own = match priority {
        Priority::Interactive => None,
        Priority::Background => background,
    };
    [global, own, client].into_iter().flatten().filter(|t| *t > now).max()
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
        self.went_back = false;
        self.covers_failed.clear();
            cx.emit(AppEvent::Navigated);
        cx.notify();
    }

    /// A página atual veio de "Voltar".
    pub fn went_back(&self) -> bool {
        self.went_back
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
            self.went_back = true;
            self.covers_failed.clear();
            cx.emit(AppEvent::Navigated);
            cx.notify();
        }
    }

    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() {
            self.index += 1;
            self.went_back = false;
            self.covers_failed.clear();
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

    /// Imagem de uma URL pública ou de um arquivo local (`file:<caminho>`, capas do
    /// fanart); começa a carregar na primeira vez.
    pub fn image(&mut self, url: &str, cx: &mut Context<Self>) -> Img {
        if let Some(i) = self.images.get(url) {
            return i;
        }
        self.images.insert(url.to_owned(), Img::Loading, 0);
        let core = self.core.clone();
        let owned = url.to_owned();
        let local = file_key_path(url);
        let task = self.core.spawn(async move {
            let bytes = match local {
                Some(path) => tokio::fs::read(path).await.map_err(|e| e.to_string()),
                None => core.fetch_public(&owned).await,
            }
            .ok()?;
            tokio::task::spawn_blocking(move || decode(&bytes, PORTRAIT_MAX, LANDSCAPE_MAX)).await.ok().flatten()
        });
        let key = url.to_owned();
        cx.spawn(async move |this, cx| {
            let result = task.await.ok().flatten();
            let _ = this.update(cx, |s, cx| {
                let (img, weight) = match result {
                    Some((image, weight)) => (Img::Ready(image), weight),
                    None => (Img::Failed, 0),
                };
                if matches!(img, Img::Failed) {
                    s.cover_file_failed(&key);
                }
                let evicted = s.images.insert(key, img, weight);
                release_images(evicted, cx);
                cx.notify();
            });
        })
        .detach();
        Img::Loading
    }

    /// Capa local que não abriu ou não decodifica: apaga o arquivo e volta ao AniList.
    fn cover_file_failed(&mut self, key: &str) {
        let Some(path) = file_key_path(key) else { return };
        self.core.fanart().discard(&path);
        if let Some(id) = self.cover_files.remove(key) {
            self.covers.insert(id, Some(None));
        }
    }

    /// Chave de imagem da capa: fanart local (`file:<caminho>`) se já existe, senão a URL
    /// do AniList. Pede o fanart uma vez por id de mídia na sessão.
    pub fn cover_key(&mut self, m: &MediaSummary, small: bool, cx: &mut Context<Self>) -> Option<String> {
        let anilist = if small { m.cover_small() } else { m.cover() };
        if m.media_type == Some(MediaType::Manga) || self.covers_failed.contains(&m.id) {
            return anilist.map(str::to_owned);
        }
        if !self.covers.contains_key(&m.id) {
            match self.core.fanart().cover_cached(m) {
                Some(p) => {
                    self.covers.insert(m.id, Some(Some(p)));
                }
                None => {
                    self.covers.insert(m.id, None);
                    let core = self.core.clone();
                    let media = m.clone();
                    let task = self.core.spawn(async move { core.fanart().cover(&media).await });
                    let id = m.id;
                    cx.spawn(async move |this, cx| {
                        let res = task.await.ok();
                        let _ = this.update(cx, |s, cx| {
                            match res {
                                Some(Ok(path)) => {
                                    s.covers.insert(id, Some(path));
                                }
                                // Rede ou tarefa perdida: AniList agora; tenta de novo na próxima navegação.
                                _ => {
                                    s.covers.remove(&id);
                                    s.covers_failed.insert(id);
                                }
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }
            }
        }
        let local = match self.covers.get(&m.id) {
            Some(Some(Some(p))) => Some(file_key(p)),
            _ => None,
        };
        if let Some(key) = local {
            // Chamado a cada quadro por card: só aloca a cópia da chave na primeira vez.
            if self.cover_files.get(&key) != Some(&m.id) {
                self.cover_files.insert(key.clone(), m.id);
            }
            return Some(key);
        }
        anilist.map(str::to_owned)
    }

    /// Guarda bytes já baixados (páginas, capas da nuvem) sob uma chave.
    /// Decodifica fora da UI; até lá a chave fica `Loading`.
    pub fn put_image(&mut self, key: String, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.images.insert_pinned(key.clone(), Img::Loading);
        let task = self.core.spawn(async move {
            tokio::task::spawn_blocking(move || decode(&bytes, PAGE_MAX, PAGE_MAX)).await.ok().flatten()
        });
        cx.spawn(async move |this, cx| {
            let result = task.await.ok().flatten();
            let _ = this.update(cx, |s, cx| {
                // Esquecida enquanto decodificava: não ressuscita.
                if s.images.get(&key).is_none() {
                    return;
                }
                let img = result.map_or(Img::Failed, |(image, _)| Img::Ready(image));
                // Bytes da nuvem: fixos fora do LRU, para não refazer a busca; saem em `forget_image`.
                let evicted = s.images.insert_pinned(key, img);
                release_images(evicted, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn cached_image(&self, key: &str) -> Option<Img> {
        self.images.get(key)
    }

    pub fn mark_image_loading(&mut self, key: &str) {
        self.images.insert(key.to_owned(), Img::Loading, 0);
    }

    pub fn fail_image(&mut self, key: &str, cx: &mut Context<Self>) {
        self.images.insert(key.to_owned(), Img::Failed, 0);
        cx.notify();
    }

    pub fn forget_image(&mut self, key: &str, cx: &mut Context<Self>) {
        release_images(self.images.remove(key).into_iter().collect(), cx);
    }

    // --- AniList em lote ---

    /// `None` = ainda buscando; `Some(None)` = sem correspondência.
    pub fn lookup(&self, name: &str, media: MediaType) -> Option<Option<Arc<MediaSummary>>> {
        self.lookups.get(media, name).cloned().flatten()
    }

    /// Muda sempre que algum nome passa a ter resposta.
    pub(crate) fn lookups_gen(&self) -> u64 {
        self.lookups_gen
    }

    /// Como `lookup`, sem copiar (para varrer o índice inteiro).
    pub fn lookup_ref(&self, name: &str, media: MediaType) -> Option<Option<&MediaSummary>> {
        self.lookups.get(media, name).and_then(Option::as_ref).map(Option::as_deref)
    }

    /// Até quando a faixa espera (falha de lote ou 429 pedido ao cliente).
    fn lane_paused(&self, priority: Priority) -> Option<Instant> {
        let client = self.core.anilist().paused_until();
        lane_paused_until(priority, Instant::now(), self.lookups_cooldown_until, self.background_cooldown_until, client)
    }

    /// Até quando o rastreio está parado por pausa: depois de um lote que falhou
    /// ou pausa pedida ao cliente pelo AniList (429). A da tela também conta.
    pub(crate) fn anilist_paused_until(&self) -> Option<Instant> {
        self.lane_paused(Priority::Background)
    }

    /// Há buscas na fila, prestes a sair, esperando resposta ou em pausa (o rastreio espera).
    pub(crate) fn lookups_busy(&self) -> bool {
        !self.lookup_queue.is_empty()
            || !self.lookup_background.is_empty()
            || self.lookup_task.is_some()
            || self.lookups_inflight > 0
            || self.anilist_paused_until().is_some()
    }

    /// Pede a busca do que a pessoa está vendo (junta pedidos próximos numa requisição só).
    /// `false` se o nome já estava pedido ou respondido.
    pub fn want_lookup(&mut self, name: &str, media: MediaType, cx: &mut Context<Self>) -> bool {
        self.want_lookup_with(name, media, Priority::Interactive, cx)
    }

    /// Como `want_lookup`, na faixa da prioridade dada (`Background` = rastreio).
    pub(crate) fn want_lookup_with(&mut self, name: &str, media: MediaType, priority: Priority, cx: &mut Context<Self>) -> bool {
        if self.lookups.contains(media, name) {
            // Esperava na fila do rastreio e agora a pessoa está vendo: passa para a frente.
            if priority == Priority::Interactive
                && let Some(pos) = self.lookup_background.iter().position(|k| k.0 == media && k.1 == name)
            {
                let key = self.lookup_background.remove(pos);
                self.lookup_queue.push(key);
                self.schedule_flush(LOOKUP_DELAY, cx);
            }
            return false;
        }
        let key = (media, name.to_owned());
        self.lookups.insert(key.clone(), None);
        self.lane(priority).push(key);
        self.schedule_flush(LOOKUP_DELAY, cx);
        true
    }

    fn lane(&mut self, priority: Priority) -> &mut Vec<(MediaType, String)> {
        match priority {
            Priority::Interactive => &mut self.lookup_queue,
            Priority::Background => &mut self.lookup_background,
        }
    }

    /// Agenda a próxima saída da fila; uma já agendada só é trocada se esta sair antes
    /// (um card visível não espera a pausa do rastreio).
    fn schedule_flush(&mut self, after: Duration, cx: &mut Context<Self>) {
        let due = Instant::now() + after;
        if self.lookup_task.is_some() && self.lookup_due.is_some_and(|d| d <= due) {
            return;
        }
        self.lookup_due = Some(due);
        self.lookup_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |s, cx| s.flush_lookups(cx));
        }));
    }

    fn flush_lookups(&mut self, cx: &mut Context<Self>) {
        self.lookup_task = None;
        self.lookup_due = None;
        let Some((priority, n)) = next_lane(self.lookup_queue.len(), self.lookup_background.len()) else { return };
        // Faixa em pausa: nada sai; a fila espera o fim dela.
        if let Some(until) = self.lane_paused(priority) {
            self.schedule_flush(until.saturating_duration_since(Instant::now()), cx);
            return;
        }
        let batch: Vec<(MediaType, String)> = self.lane(priority).drain(..n).collect();
        if !self.lookup_queue.is_empty() || !self.lookup_background.is_empty() {
            self.schedule_flush(LOOKUP_DELAY, cx);
        }
        let core = self.core.clone();
        let items: Vec<LookupRequest> = batch.iter().map(|(m, n)| LookupRequest { name: n.clone(), media_type: *m }).collect();
        let task = self.core.spawn(async move { core.anilist().lookup_with(&items, priority).await });
        self.lookups_inflight += 1;
        cx.spawn(async move |this, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |s, cx| {
                s.lookups_inflight = s.lookups_inflight.saturating_sub(1);
                let anime = |key: &(MediaType, String)| Some(key.clone());
                match result {
                    Some(list) => {
                        s.lookups_gen += 1;
                        let mut pending = Vec::new();
                        let mut answered = Vec::new();
                        // Progresso: algum nome que já tinha voltado adiado agora tem resposta.
                        let mut progress = false;
                        for (key, v) in batch.into_iter().zip(list) {
                            if !matches!(v, Lookup::Pending) {
                                answered.extend(anime(&key));
                                progress |= s.lookup_retries.remove(&key).is_some();
                            }
                            match v {
                                Lookup::Found(v) => {
                                    s.lookups.insert(key, Some(serde_json::from_value::<MediaSummary>(v).ok().map(Arc::new)));
                                }
                                Lookup::NotFound => {
                                    s.lookups.insert(key, Some(None));
                                }
                                // Fallback MAL adiado ou parte do lote falhou: continua "buscando".
                                Lookup::Pending => pending.push(key),
                            }
                        }
                        // Volta à frente da mesma faixa depois da espera; sem progresso por
                        // várias vezes, desiste (um card visível pode pedir de novo depois).
                        let mut later: HashMap<Duration, Vec<(MediaType, String)>> = HashMap::new();
                        let mut dropped = Vec::new();
                        for key in pending {
                            match next_retry(s.lookup_retries.get(&key).copied(), progress) {
                                Some((n, wait)) => {
                                    s.lookup_retries.insert(key.clone(), n);
                                    later.entry(wait).or_default().push(key);
                                }
                                None => {
                                    s.lookup_retries.remove(&key);
                                    dropped.extend(anime(&key));
                                    s.lookups.remove(&key);
                                }
                            }
                        }
                        for (wait, retry) in later {
                            cx.spawn(async move |this, cx| {
                                cx.background_executor().timer(wait).await;
                                let _ = this.update(cx, |s, cx| {
                                    s.lane(priority).splice(0..0, retry);
                                    // Antecipa uma saída agendada para o fim da pausa do rastreio.
                                    s.schedule_flush(LOOKUP_DELAY, cx);
                                });
                            })
                            .detach();
                        }
                        s.crawl_lookups_done(&answered, false);
                        // O rastreio não pede de novo os que desistiu (não volta à fila dele).
                        s.crawl_lookups_done(&dropped, false);
                    }
                    // Falha de rede/limite: libera os nomes e pausa a faixa (sem isso os cards
                    // visíveis pediriam de novo no quadro seguinte). Falha do rastreio só pausa
                    // o rastreio; falha da tela pausa as duas. O rastreio pede de novo os dele.
                    None => {
                        let end = Some(cooldown_end(Instant::now(), s.core.anilist().paused_until()));
                        match priority {
                            Priority::Interactive => s.lookups_cooldown_until = end,
                            Priority::Background => s.background_cooldown_until = end,
                        }
                        let failed: Vec<(MediaType, String)> = batch.iter().filter_map(anime).collect();
                        for key in batch {
                            s.lookup_retries.remove(&key);
                            s.lookups.remove(&key);
                        }
                        s.crawl_lookups_done(&failed, true);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // --- Episódios (TMDB) e preferências de exibição ---

    /// `None` = buscando ou não pedido; `Some(None)` = sem TMDB para a obra.
    pub fn episodes(&self, key: &str) -> Option<Option<Arc<WorkEpisodes>>> {
        if self.episodes_failed.contains(key) {
            return Some(None);
        }
        self.episodes.get(key).cloned().flatten()
    }

    /// Pede os episódios da obra uma vez por chave. TMDB sem token ou sem a obra
    /// → `Some(None)` até o fim da sessão. Falha passageira (rede, 429) não fica:
    /// a obra aparece sem dados e o próximo pedido (outra visita à página; a tela
    /// pede uma vez por visita) tenta de novo.
    pub fn want_episodes(&mut self, key: String, q: WorkQuery, cx: &mut Context<Self>) {
        if self.episodes.contains_key(&key) {
            return;
        }
        self.episodes_failed.remove(&key);
        if !self.core.tmdb().configured() {
            self.episodes.insert(key, Some(None));
            cx.notify();
            return;
        }
        self.episodes.insert(key.clone(), None);
        let core = self.core.clone();
        let task = self.core.spawn(async move { core.tmdb().work_episodes(&q).await });
        cx.spawn(async move |this, cx| {
            let outcome = episodes_outcome(task.await.ok());
            let _ = this.update(cx, |s, cx| {
                match outcome {
                    Some(found) => {
                        s.episodes.insert(key, Some(found));
                    }
                    None => {
                        s.episodes.remove(&key);
                        s.episodes_failed.insert(key);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Layout da aba Episódios (grade ou lista), guardado nas preferências.
    pub fn episode_layout(&self) -> EpisodeLayout {
        self.core.settings().episode_layout()
    }

    pub fn set_episode_layout(&mut self, layout: EpisodeLayout, cx: &mut Context<Self>) {
        self.core.settings().set_episode_layout(layout);
        cx.notify();
    }

    /// Fonte das notas escolhida nas preferências.
    pub fn rating_source(&self) -> &'static dyn RatingSource {
        ratings::source(self.core.settings().rating_provider())
    }

    // --- Avisos ---

    // --- Barra lateral ---

    /// Recolhida (só ícones); guardada nas configurações.
    pub fn sidebar_collapsed(&self) -> bool {
        self.core.settings().sidebar_collapsed()
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        let collapsed = !self.sidebar_collapsed();
        self.core.settings().set_sidebar_collapsed(collapsed);
        cx.notify();
    }

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
        let (done, total) = self
            .downloads
            .iter()
            .filter(|d| d.status == DlStatus::Running)
            .filter_map(|d| Some((d.downloaded, d.total?)))
            .fold((0u64, 0u64), |(a, b), (d, t)| (a + d, b + t));
        if total == 0 { 0. } else { done as f32 / total as f32 }
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
        self.downloads.insert(0, Download { id, path: path.to_owned(), name, downloaded: 0, total: None, status: DlStatus::Queued, dest: None, error: None, abort: None });
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
            if let Some(d) = self.downloads.iter_mut().find(|d| d.id == id) {
                d.abort = Some(task.abort_handle());
            }
            cx.spawn(async move |this, cx| {
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r.map_err(|e| e.to_string()));
                let _ = this.update(cx, |s, cx| {
                    // Cancelado: a linha já saiu da lista e não há o que avisar.
                    let Some(name) = s.downloads.iter().find(|d| d.id == id).map(|d| d.name.clone()) else {
                        return;
                    };
                    if let Some(d) = s.downloads.iter_mut().find(|d| d.id == id) {
                        d.abort = None;
                    }
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

    /// Tira o download da fila ou interrompe o que está baixando (o núcleo
    /// apaga o `.part`), e começa o próximo da fila.
    pub fn cancel_download(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(i) = self.downloads.iter().position(|d| d.id == id) else { return };
        if !matches!(self.downloads[i].status, DlStatus::Running | DlStatus::Queued) {
            return;
        }
        if let Some(abort) = self.downloads.remove(i).abort {
            abort.abort();
        }
        self.pump(cx);
        cx.notify();
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

/// Resultado da busca no TMDB para o mapa de episódios: achou ou não tem a obra
/// → `Some`; erro (rede, 429) ou tarefa perdida → `None`, para não guardar.
fn episodes_outcome<T, E>(r: Option<Result<Option<T>, E>>) -> Option<Option<Arc<T>>> {
    match r {
        Some(Ok(found)) => Some(found.map(Arc::new)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_lane_goes_first() {
        assert_eq!(next_lane(0, 0), None);
        assert_eq!(next_lane(5, 100), Some((Priority::Interactive, 5)));
        assert_eq!(next_lane(50, 0), Some((Priority::Interactive, 36)));
        // O rastreio só sai com a faixa da pessoa vazia, um lote de uma requisição.
        assert_eq!(next_lane(0, 7), Some((Priority::Background, 7)));
        assert_eq!(next_lane(0, 100), Some((Priority::Background, 18)));
    }

    #[test]
    fn crawl_failure_pauses_only_its_lane() {
        let now = Instant::now();
        let (i, b) = (Priority::Interactive, Priority::Background);
        let later = |s| Some(now + Duration::from_secs(s));
        assert_eq!(lane_paused_until(i, now, None, None, None), None);
        // Lote do rastreio falhou: os cards da tela seguem; o rastreio espera.
        assert_eq!(lane_paused_until(i, now, None, later(30), None), None);
        assert_eq!(lane_paused_until(b, now, None, later(30), None), later(30));
        // Lote da tela falhou: as duas faixas esperam.
        assert_eq!(lane_paused_until(i, now, later(30), None, None), later(30));
        assert_eq!(lane_paused_until(b, now, later(30), later(10), None), later(30));
        // 429 no cliente vale para todos; pausa vencida não conta.
        assert_eq!(lane_paused_until(i, now, Some(now - Duration::from_secs(1)), None, later(12)), later(12));
        assert_eq!(lane_paused_until(b, now, None, Some(now), None), None);
    }

    #[test]
    fn pending_retries_back_off_then_give_up() {
        let s = Duration::from_secs;
        // Sem progresso: 3, 6, 12, 24, 48 s e desiste.
        assert_eq!(next_retry(None, false), Some((0, s(3))));
        assert_eq!(next_retry(Some(0), false), Some((1, s(6))));
        assert_eq!(next_retry(Some(1), false), Some((2, s(12))));
        assert_eq!(next_retry(Some(2), false), Some((3, s(24))));
        assert_eq!(next_retry(Some(3), false), Some((4, s(48))));
        assert_eq!(next_retry(Some(4), false), None);
        // O lote andou (fallback MAL resolvendo aos poucos): a conta recomeça.
        assert_eq!(next_retry(Some(4), true), Some((0, s(3))));
    }

    #[test]
    fn cooldown_takes_the_longest_pause() {
        let now = Instant::now();
        assert_eq!(cooldown_end(now, None), now + Duration::from_secs(30));
        assert_eq!(cooldown_end(now, Some(now + Duration::from_secs(5))), now + Duration::from_secs(30));
        assert_eq!(cooldown_end(now, Some(now + Duration::from_secs(61))), now + Duration::from_secs(61));
    }

    #[test]
    fn file_keys_roundtrip() {
        let p = PathBuf::from("covers").join("a b.jpg");
        let key = file_key(&p);
        assert!(key.starts_with("file:"));
        assert_eq!(file_key_path(&key), Some(p));
        assert_eq!(file_key_path("https://x/y.jpg"), None);
    }

    #[test]
    fn tmdb_failure_is_not_remembered() {
        // Achou ou o TMDB não tem a obra: fica para a sessão.
        assert_eq!(episodes_outcome(Some(Ok::<_, ()>(Some(7)))), Some(Some(Arc::new(7))));
        assert_eq!(episodes_outcome(Some(Ok::<Option<u8>, ()>(None))), Some(None));
        // Rede, 429 ou tarefa perdida: não guarda; a próxima visita tenta de novo.
        assert_eq!(episodes_outcome(Some(Err::<Option<u8>, _>(()))), None);
        assert_eq!(episodes_outcome(None::<Result<Option<u8>, ()>>), None);
    }
}
