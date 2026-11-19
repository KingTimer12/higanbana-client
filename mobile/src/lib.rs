//! Ponte entre as telas TSX e o `higanbana-core`: o que `App.tsx` importa de
//! `higanbana/native`. As declarações TypeScript são geradas destas assinaturas
//! em `.nsh/types/higanbana/native.d.ts`.
//!
//! A sessão nunca chega ao TSX: daqui saem só nomes, caminhos e estados.

mod library;
mod offline;
mod player;
mod reader;
mod updates;

use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use higanbana_core::{
    Core, Paths,
    anilist::{
        Lookup, LookupRequest, MediaType, Ranking,
        types::{MediaSummary, RankedMedia},
    },
    anime_index::LETTERS,
    auth::{AuthError, Session},
    events,
    library::{folder_year, parse_work_name},
    nuvem::SearchKind,
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

pub use library::Library;
pub use offline::Offline;
pub use player::Player;
pub use reader::Reader;
pub use updates::Updates;

/// O que `App.tsx` importa de `higanbana/native`.
#[native_module("higanbana/native")]
fn native(registry: &mut Registry) {
    registry
        .module::<Account>()
        .module::<Catalog>()
        .module::<Library>()
        .module::<Offline>()
        .module::<Player>()
        .module::<Reader>()
        .module::<Updates>();
}

static CORE: OnceLock<Arc<Core>> = OnceLock::new();

/// O núcleo, depois de `Account.start`.
pub(crate) fn core() -> Result<Arc<Core>, String> {
    CORE.get().cloned().ok_or_else(|| "núcleo não iniciado".to_owned())
}

/// `file://` para o `Image` do NSH (que também lê URIs de arquivo): barras para frente,
/// e `%`, `#` e `?` escapados para não virarem parte da URL.
fn file_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    // Uma passada só, numa String já do tamanho certo (uma capa por card).
    let mut uri = String::with_capacity(path.len() + 16);
    uri.push_str(if path.starts_with('/') || path.starts_with('\\') { "file://" } else { "file:///" });
    for c in path.chars() {
        match c {
            '\\' => uri.push('/'),
            '%' => uri.push_str("%25"),
            '#' => uri.push_str("%23"),
            '?' => uri.push_str("%3F"),
            c => uri.push(c),
        }
    }
    uri
}

/// Roda `f` no runtime do núcleo e espera o resultado na thread da interface.
pub(crate) async fn on_core<F, Fut, T>(f: F) -> Result<T, String>
where
    F: FnOnce(Arc<Core>) -> Fut,
    Fut: Future<Output = Result<T, String>> + Send + 'static,
    T: Send + 'static,
{
    let core = core()?;
    core.spawn(f(core.clone())).await.map_err(|e| e.to_string())?
}

/// Nome e avatar do Discord, que é o que a interface mostra da sessão.
fn user_of(session: &Session) -> JsonValue {
    let meta = session.user.as_ref().and_then(|u| u.get("user_metadata"));
    let field = |keys: &[&str]| keys.iter().find_map(|k| meta.and_then(|m| m.get(k)).and_then(JsonValue::as_str)).map(str::to_owned);
    json!({
        "name": field(&["full_name", "name", "user_name"]).unwrap_or_else(|| "Você".to_owned()),
        "avatar": field(&["avatar_url", "picture"]),
    })
}

/// A pasta que o release recebe como `process.dataDir`: o NSH põe
/// `nsh/<id do app>` dentro do `setStorageDirectory(getFilesDir())` do host.
/// O pacote sai do nome do processo (no Android, o do pacote; aqui igual ao id
/// do `nsh.json`). Fora do Android, `None`.
fn android_files_dir() -> Option<String> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let cmdline = std::fs::read("/proc/self/cmdline").ok()?;
    let name = String::from_utf8_lossy(cmdline.split(|b| *b == 0).next()?).into_owned();
    // Processos secundários se chamam `pacote:nome`; a pasta é a do pacote.
    let package = name.split(':').next().filter(|p| !p.is_empty() && !p.contains('/'))?;
    Some(format!("/data/user/0/{package}/files/nsh/{package}"))
}

/// Sessão do Anitsu (login pelo Discord).
#[module]
pub struct Account;

#[module]
impl Account {
    /// Inicia o núcleo. `dataDir` é a pasta de dados do app (`process.dataDir`);
    /// sem ela, usa as pastas padrão do sistema (desktop). Chamar de novo não faz nada.
    pub fn start(data_dir: Option<String>) -> Result<(), String> {
        if CORE.get().is_some() {
            return Ok(());
        }
        // A sessão do `nsh dev` no Android não recebe a pasta do host
        // (`setStorageDirectory` só vale para o bundle embutido): sem ela, a
        // mesma pasta que o `getFilesDir()` dá, para dev e release dividirem a sessão.
        let data_dir = data_dir.filter(|d| !d.is_empty()).or_else(android_files_dir);
        // Thread própria: o núcleo cria um runtime tokio e bloqueia nele ao subir o proxy.
        let core = std::thread::spawn(move || {
            let paths = match data_dir {
                Some(dir) => Paths::under(dir),
                None => Paths::system(),
            }
            .map_err(|e| e.to_string())?;
            Core::new(paths, events::ignore()).map_err(|e| e.to_string())
        })
        .join()
        .map_err(|_| "falha ao iniciar o núcleo".to_owned())??;
        let _ = CORE.set(Arc::new(core));
        Ok(())
    }

    /// Quem está logado (`{ name, avatar }`), ou `null`.
    pub async fn user() -> Result<JsonValue, String> {
        on_core(|c| async move {
            let session = match c.auth().session().await {
                Ok(session) => session,
                // Sem rede, a renovação não sai: quem estava logado continua, e os baixados abrem.
                Err(AuthError::Unreachable(_) | AuthError::Network(_)) => c.auth().stored().await,
                Err(e) => return Err(e.to_string()),
            };
            Ok(session.as_ref().map_or(JsonValue::Null, user_of))
        })
        .await
    }

    /// Começa o login e resolve com o usuário quando o navegador volta.
    /// A página a abrir sai em `loginUrl`; rejeita se cancelado ou expirado.
    pub async fn login() -> Result<JsonValue, String> {
        on_core(|c| async move {
            let session = c.auth().login_with(|_| Ok(())).await.map_err(|e| e.to_string())?;
            Ok(user_of(&session))
        })
        .await
    }

    /// Página de login em andamento, para abrir no navegador; `null` fora de um login.
    pub async fn login_url() -> Result<Option<String>, String> {
        on_core(|c| async move { Ok(c.auth().pending_login_url().await) }).await
    }

    /// Desiste do login em andamento.
    pub async fn cancel_login() -> Result<(), String> {
        on_core(|c| async move {
            c.auth().cancel_login().await;
            Ok(())
        })
        .await
    }

    /// Encerra a sessão.
    pub async fn logout() -> Result<(), String> {
        on_core(|c| async move { c.auth().logout().await.map_err(|e| e.to_string()) }).await
    }
}

/// A seção do acervo que a tela pede: `"anime"` ou `"manga"`.
fn media_type(kind: &str) -> Result<MediaType, String> {
    match kind {
        "anime" => Ok(MediaType::Anime),
        "manga" => Ok(MediaType::Manga),
        other => Err(format!("seção desconhecida: {other}")),
    }
}

/// Descoberta de obras: vitrines do AniList, busca e índice A–Z da nuvem,
/// para os animes e os mangás (`kind`).
#[module]
pub struct Catalog;

#[module]
impl Catalog {
    /// Caminho da obra na nuvem para um resumo do AniList; `null` se não está no acervo.
    pub async fn find(media: JsonValue) -> Result<Option<String>, String> {
        let media: MediaSummary = serde_json::from_value(media).map_err(|e| e.to_string())?;
        on_core(|c| async move { Ok(c.check_catalog(&media).await) }).await
    }

    /// URI do pôster local do fanart (`file://…`), ou `null` (usar o AniList).
    pub async fn cover(media: JsonValue) -> Result<Option<String>, String> {
        let media: MediaSummary = serde_json::from_value(media).map_err(|e| e.to_string())?;
        on_core(|c| async move {
            // `Ok(None)` (sem chave, sem pôster) = usar o AniList; erro de rede volta como erro
            // para a tela liberar o id e tentar de novo depois.
            let path = match c.fanart().cover_cached(&media) {
                Some(path) => Some(path),
                None => c.fanart().cover(&media).await.map_err(|e| e.to_string())?,
            };
            Ok(path.map(|p| file_uri(&p)))
        })
        .await
    }

    /// Detalhe completo de uma mídia do AniList (sinopse, estúdios, relações).
    pub async fn media(id: f64) -> Result<JsonValue, String> {
        on_core(move |c| async move { c.anilist().media(id as i64).await.map_err(|e| e.to_string()) }).await
    }

    /// Casa nomes de pasta com o AniList: um resumo (ou `null`) por nome, na mesma ordem.
    pub async fn lookup(names: Vec<String>, kind: String) -> Result<JsonValue, String> {
        let media_type = media_type(&kind)?;
        on_core(move |c| async move {
            let items: Vec<LookupRequest> = names.into_iter().map(|name| LookupRequest { name, media_type }).collect();
            let found = c.anilist().lookup(&items).await.map_err(|e| e.to_string())?;
            Ok(found
                .into_iter()
                .map(|l| match l {
                    Lookup::Found(media) => media,
                    Lookup::NotFound | Lookup::Pending => JsonValue::Null,
                })
                .collect())
        })
        .await
    }

    /// Busca obras da seção na nuvem pelo nome: `[{ name, path, title, year }]`.
    pub async fn search(query: String, kind: String) -> Result<JsonValue, String> {
        let wanted = match media_type(&kind)? {
            MediaType::Anime => SearchKind::Anime,
            MediaType::Manga => SearchKind::Manga,
        };
        on_core(move |c| async move {
            let found = c.cloud().search(&query).await.map_err(|e| e.to_string())?;
            Ok(found.results.into_iter().filter(|r| r.kind == wanted).map(|r| work_entry(&r.name, &r.path)).collect())
        })
        .await
    }

    /// Começa uma vitrine do início, como no desktop: um ranking de animes do AniList
    /// (`"popular"`, `"topRated"` ou `"trending"`) só com o que está no acervo, até `n`
    /// obras, na ordem do ranking. Devolve o número da conferência para
    /// [`Catalog::discover_poll`]: cada obra aparece lá assim que é confirmada,
    /// sem esperar a lista inteira (a conferência de uma obra leva quase um segundo).
    pub fn discover_start(ranking: String, n: f64) -> Result<f64, String> {
        let ranking = match ranking.as_str() {
            "popular" => Ranking::Popular,
            "topRated" => Ranking::TopRated,
            "trending" => Ranking::Trending,
            other => return Err(format!("ranking desconhecido: {other}")),
        };
        let n = n.clamp(1.0, 24.0) as usize;
        let c = core()?;
        let id = NEXT_DISCOVERY.fetch_add(1, Ordering::Relaxed);
        let found = Arc::new(Mutex::new(Discovery::default()));
        discoveries().insert(id, found.clone());
        let task = c.clone();
        c.spawn(async move {
            let into = found.clone();
            task.discover(MediaType::Anime, ranking, n, task.home_budget(), move |m, path| {
                into.lock().unwrap().items.push(json!({ "media": ranked_json(&m), "path": path }));
            })
            .await;
            found.lock().unwrap().done = true;
        });
        Ok(id as f64)
    }

    /// O que a conferência `id` já achou: `{ items, done }`. Depois de `done`, esquece a conferência.
    pub fn discover_poll(id: f64) -> Result<JsonValue, String> {
        let id = id as u64;
        let mut all = discoveries();
        let Some(found) = all.get(&id).cloned() else {
            return Err("conferência desconhecida".to_owned());
        };
        let found = found.lock().unwrap();
        if found.done {
            all.remove(&id);
        }
        Ok(json!({ "items": found.items, "done": found.done }))
    }

    /// As pastas de letra do catálogo (`0-9`, `Letra A`, ...).
    pub fn letters() -> Vec<String> {
        LETTERS.iter().map(|l| (*l).to_owned()).collect()
    }

    /// Obras de uma letra da seção: `[{ name, path, title, year }]`.
    pub async fn letter(letter: String, kind: String) -> Result<JsonValue, String> {
        let media_type = media_type(&kind)?;
        on_core(move |c| async move {
            let entries = c.section_index(media_type).letter(c.cloud(), &letter).await?;
            Ok(entries.iter().map(|e| work_entry(&e.name, &e.path)).collect())
        })
        .await
    }
}

/// Uma vitrine em conferência: as obras já confirmadas e se terminou.
#[derive(Default)]
struct Discovery {
    items: Vec<JsonValue>,
    done: bool,
}

static NEXT_DISCOVERY: AtomicU64 = AtomicU64::new(1);

fn discoveries() -> MutexGuard<'static, HashMap<u64, Arc<Mutex<Discovery>>>> {
    static ALL: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Discovery>>>>> = OnceLock::new();
    ALL.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Uma obra de ranking no formato `Media` das telas (os campos do AniList em camelCase),
/// com as três tags mais relevantes sem spoiler e o estúdio principal.
fn ranked_json(m: &RankedMedia) -> JsonValue {
    let s = &m.summary;
    json!({
        "id": s.id,
        "idMal": s.id_mal,
        "type": "ANIME",
        "format": s.format,
        "status": s.status,
        "title": {
            "romaji": s.title.romaji,
            "english": s.title.english,
            "native": s.title.native,
            "userPreferred": s.title.user_preferred,
        },
        "coverImage": s.cover_image.as_ref().map(|c| json!({ "extraLarge": c.extra_large, "large": c.large, "color": c.color })),
        "bannerImage": s.banner_image,
        "averageScore": s.average_score,
        "genres": s.genres,
        "episodes": s.episodes,
        "seasonYear": s.year(),
        "description": m.description,
        "tags": m.top_tags(3),
        "studio": m.studios.as_ref().and_then(|st| st.nodes.first()).map(|n| n.name.clone()),
    })
}

/// Uma pasta de obra como a lista mostra.
fn work_entry(name: &str, path: &str) -> JsonValue {
    json!({ "name": name, "path": path, "title": parse_work_name(name).title, "year": folder_year(name) })
}

#[cfg(test)]
mod tests {
    use super::file_uri;
    use std::path::Path;

    #[test]
    fn file_uri_do_android() {
        assert_eq!(file_uri(Path::new("/data/user/0/x/files/covers/ab.webp")), "file:///data/user/0/x/files/covers/ab.webp");
    }

    #[test]
    fn file_uri_do_windows() {
        assert_eq!(file_uri(Path::new(r"D:\x\y.webp")), "file:///D:/x/y.webp");
    }
}
