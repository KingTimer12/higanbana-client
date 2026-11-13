//! Ponte entre as telas TSX e o `higanbana-core`: o que `App.tsx` importa de
//! `higanbana/native`. As declarações TypeScript são geradas destas assinaturas
//! em `.nsh/types/higanbana/native.d.ts`.
//!
//! A sessão nunca chega ao TSX: daqui saem só nomes, caminhos e estados.

mod library;
mod player;

use std::{
    future::Future,
    sync::{Arc, OnceLock},
};

use higanbana_core::{
    Core, Paths,
    anilist::{Lookup, LookupRequest, MediaType, types::MediaSummary},
    anime_index::LETTERS,
    auth::Session,
    events,
    library::{folder_year, parse_work_name},
    nuvem::SearchKind,
};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

pub use library::Library;
pub use player::Player;

/// O que `App.tsx` importa de `higanbana/native`.
#[native_module("higanbana/native")]
fn native(registry: &mut Registry) {
    registry.module::<Account>().module::<Catalog>().module::<Library>().module::<Player>();
}

static CORE: OnceLock<Arc<Core>> = OnceLock::new();

/// O núcleo, depois de `Account.start`.
pub(crate) fn core() -> Result<Arc<Core>, String> {
    CORE.get().cloned().ok_or_else(|| "núcleo não iniciado".to_owned())
}

/// `file://` para o `Image` do NSH (que também lê URIs de arquivo): barras para frente,
/// e `%`, `#` e `?` escapados para não virarem parte da URL.
fn file_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/").replace('%', "%25").replace('#', "%23").replace('?', "%3F");
    if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") }
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
    let field = |keys: &[&str]| {
        keys.iter().find_map(|k| meta.and_then(|m| m.get(k)).and_then(JsonValue::as_str)).map(str::to_owned)
    };
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
            let session = c.auth().session().await.map_err(|e| e.to_string())?;
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

/// Descoberta de animes: temporada do AniList, busca e índice A–Z da nuvem.
#[module]
pub struct Catalog;

#[module]
impl Catalog {
    /// Animes da temporada atual (resumos do AniList).
    pub async fn season() -> Result<JsonValue, String> {
        on_core(|c| async move { c.anilist().season().await.map_err(|e| e.to_string()) }).await
    }

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
    pub async fn lookup(names: Vec<String>) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let items: Vec<LookupRequest> =
                names.into_iter().map(|name| LookupRequest { name, media_type: MediaType::Anime }).collect();
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

    /// Busca animes na nuvem pelo nome: `[{ name, path, title, year }]`.
    pub async fn search(query: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let found = c.cloud().search(&query).await.map_err(|e| e.to_string())?;
            Ok(found
                .results
                .into_iter()
                .filter(|r| r.kind == SearchKind::Anime)
                .map(|r| work_entry(&r.name, &r.path))
                .collect())
        })
        .await
    }

    /// As pastas de letra do catálogo (`0-9`, `Letra A`, ...).
    pub fn letters() -> Vec<String> {
        LETTERS.iter().map(|l| (*l).to_owned()).collect()
    }

    /// Obras de uma letra: `[{ name, path, title, year }]`.
    pub async fn letter(letter: String) -> Result<JsonValue, String> {
        on_core(|c| async move {
            let entries = c.anime_index().letter(c.cloud(), &letter).await?;
            Ok(entries.iter().map(|e| work_entry(&e.name, &e.path)).collect())
        })
        .await
    }
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
