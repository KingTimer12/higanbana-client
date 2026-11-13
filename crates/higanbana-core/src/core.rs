//! Monta todos os serviços e o runtime assíncrono deles.

use std::{future::Future, sync::Arc};

use tokio::runtime::{Handle, Runtime};

#[cfg(feature = "music")]
use crate::music::Music;
#[cfg(feature = "reader")]
use crate::reader::Reader;
use crate::{
    anilist::AniList, fanart::Fanart, tmdb::Tmdb, anime_index::AnimeIndex, auth::AuthManager, catalog::Catalog, cloud::Cloud, events::EventFn, paths::Paths, player::Player, prefs::Prefs, settings::Settings,
};

/// Tudo que a interface usa. Os métodos `async` rodam no runtime de [`Core::rt`]
/// (use [`Core::spawn`] ou [`Core::block_on`]).
pub struct Core {
    rt: Runtime,
    cloud: Arc<Cloud>,
    anilist: AniList,
    tmdb: Tmdb,
    fanart: Fanart,
    catalog: Catalog,
    anime_index: AnimeIndex,
    prefs: Prefs,
    settings: Settings,
    web: reqwest::Client,
    #[cfg(feature = "reader")]
    reader: Reader,
    player: Player,
    #[cfg(feature = "music")]
    music: Music,
}

impl Core {
    pub fn new(paths: Paths, events: EventFn) -> Result<Self, Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Builder::new_multi_thread().thread_name("higanbana-core").enable_all().build()?;
        let cloud = Arc::new(Cloud::new(paths.clone(), events.clone())?);
        let anilist = AniList::new(&paths.cache);
        let tmdb = Tmdb::new(&paths.cache);
        let fanart = Fanart::new(&paths.cache);
        let catalog = Catalog::open(paths.cache.join("catalog.json"));
        let anime_index = AnimeIndex::open(paths.cache.join("anime-index.json"));
        let prefs = Prefs::open(paths.data.join("prefs.json"));
        let settings = Settings::open(paths.data.join("settings.json"));
        let web = reqwest::Client::builder().user_agent(concat!("higanbana-client/", env!("CARGO_PKG_VERSION"))).build()?;
        #[cfg(feature = "reader")]
        let reader = Reader::new(cloud.clone());
        let player = rt.block_on(Player::start(cloud.clone()))?;
        #[cfg(feature = "music")]
        let music = Music::start(rt.handle().clone(), cloud.clone(), events);
        #[cfg(not(feature = "music"))]
        drop(events);
        Ok(Self {
            rt,
            cloud,
            anilist,
            tmdb,
            fanart,
            catalog,
            anime_index,
            prefs,
            settings,
            web,
            #[cfg(feature = "reader")]
            reader,
            player,
            #[cfg(feature = "music")]
            music,
        })
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

    pub fn tmdb(&self) -> &Tmdb {
        &self.tmdb
    }

    pub fn fanart(&self) -> &Fanart {
        &self.fanart
    }

    pub fn anilist(&self) -> &AniList {
        &self.anilist
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
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

    #[cfg(feature = "reader")]
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    #[cfg(feature = "music")]
    pub fn music(&self) -> &Music {
        &self.music
    }
}
