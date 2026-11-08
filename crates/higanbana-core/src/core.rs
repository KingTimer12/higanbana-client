//! Monta todos os serviços e o runtime assíncrono deles.

use std::{future::Future, sync::Arc};

use tokio::runtime::{Handle, Runtime};

use crate::{
    anilist::AniList, anime_index::AnimeIndex, auth::AuthManager, catalog::Catalog, cloud::Cloud, events::EventFn, music::Music, paths::Paths, player::Player,
    reader::Reader,
};

/// Tudo que a interface usa. Os métodos `async` rodam no runtime de [`Core::rt`]
/// (use [`Core::spawn`] ou [`Core::block_on`]).
pub struct Core {
    rt: Runtime,
    cloud: Arc<Cloud>,
    anilist: AniList,
    catalog: Catalog,
    anime_index: AnimeIndex,
    web: reqwest::Client,
    reader: Reader,
    player: Player,
    music: Music,
}

impl Core {
    pub fn new(paths: Paths, events: EventFn) -> Result<Self, Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Builder::new_multi_thread().thread_name("higanbana-core").enable_all().build()?;
        let cloud = Arc::new(Cloud::new(paths.clone(), events.clone())?);
        let anilist = AniList::new(&paths.cache);
        let catalog = Catalog::open(paths.cache.join("catalog.json"));
        let anime_index = AnimeIndex::open(paths.cache.join("anime-index.json"));
        let web = reqwest::Client::builder().user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION"))).build()?;
        let reader = Reader::new(cloud.clone());
        let player = rt.block_on(Player::start(cloud.clone()))?;
        let music = Music::start(rt.handle().clone(), cloud.clone(), events);
        Ok(Self { rt, cloud, anilist, catalog, anime_index, web, reader, player, music })
    }

    pub fn rt(&self) -> &Handle {
        self.rt.handle()
    }

    pub fn spawn<F>(&self, f: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.rt.spawn(f)
    }

    /// Não chamar de dentro do runtime.
    pub fn block_on<F: Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }

    pub fn cloud(&self) -> &Arc<Cloud> {
        &self.cloud
    }

    pub fn auth(&self) -> &AuthManager {
        &self.cloud.auth
    }

    pub fn paths(&self) -> &Paths {
        &self.cloud.paths
    }

    pub fn anilist(&self) -> &AniList {
        &self.anilist
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn anime_index(&self) -> &AnimeIndex {
        &self.anime_index
    }

    /// Bytes de uma URL pública (capas do AniList, avatar do Discord). Só https.
    pub async fn fetch_public(&self, url: &str) -> Result<Vec<u8>, String> {
        if !url.starts_with("https://") {
            return Err("endereço não permitido".into());
        }
        let resp = self.web.get(url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
    }

    /// Confere se o anime está no acervo (ver [`Catalog`]).
    pub async fn check_catalog(&self, m: &crate::anilist::types::MediaSummary) -> Option<String> {
        self.catalog.check(&self.anime_index, &self.cloud, &self.anilist, m).await
    }

    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    pub fn music(&self) -> &Music {
        &self.music
    }
}
