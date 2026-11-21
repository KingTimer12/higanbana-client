//! Imagens da interface (capas, avatares): baixadas e decodificadas no núcleo,
//! guardadas num LRU com teto de memória. Soltar o `ImageSource` libera a textura.

use higanbana_gpui::{ImageData, ImageSource};

use super::AppState;
use crate::image_cache::{decode, Decoded, ImageCache, IMAGE_BUDGET, LANDSCAPE_MAX, PORTRAIT_MAX};

#[derive(Clone)]
pub enum Img {
    Loading,
    Ready(ImageSource),
    Failed,
}

pub(super) struct Images {
    cache: ImageCache<Img>,
}

impl Default for Images {
    fn default() -> Self {
        Images { cache: ImageCache::new(IMAGE_BUDGET) }
    }
}

/// Chave de imagem de um arquivo local.
pub(crate) fn file_key(path: &std::path::Path) -> String {
    format!("file:{}", path.display())
}

/// Caminho de uma chave `file:`; `None` para URLs.
pub(crate) fn file_key_path(key: &str) -> Option<std::path::PathBuf> {
    key.strip_prefix("file:").map(std::path::PathBuf::from)
}

fn to_source(d: Decoded) -> ImageSource {
    ImageData::from_rgba8(d.width, d.height, d.rgba)
}

impl AppState {
    /// Imagem de uma URL pública ou de um arquivo local (`file:<caminho>`);
    /// começa a carregar na primeira vez (pode ser chamada no render).
    pub fn image(&mut self, url: &str) -> Img {
        if let Some(i) = self.images.cache.get(url) {
            return i;
        }
        self.images.cache.insert(url.to_owned(), Img::Loading, 0);
        let core = self.core.clone();
        let (owned, local) = (url.to_owned(), file_key_path(url));
        let fut = async move {
            let bytes = match local {
                Some(path) => tokio::fs::read(path).await.map_err(|e| e.to_string()),
                None => core.fetch_public(&owned).await,
            }
            .ok()?;
            tokio::task::spawn_blocking(move || decode(&bytes, PORTRAIT_MAX, LANDSCAPE_MAX)).await.ok().flatten()
        };
        let key = url.to_owned();
        self.run(fut, move |s, result, _| {
            let (img, weight) = match result {
                Some((d, weight)) => (Img::Ready(to_source(d)), weight),
                None => (Img::Failed, 0),
            };
            s.images.cache.insert(key, img, weight);
            tracing::trace!(total_mb = s.images.cache.total_mb(), n = s.images.cache.len(), "cache de imagens");
        });
        Img::Loading
    }

    pub fn cached_image(&self, key: &str) -> Option<Img> {
        self.images.cache.get(key)
    }

    /// Solta as imagens da interface (ao abrir o player, por exemplo).
    pub fn release_all_images(&mut self) {
        let n = self.images.cache.drain_weighted().len();
        tracing::debug!(n, "imagens soltas");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_keys_roundtrip() {
        let p = std::path::PathBuf::from("covers").join("a b.jpg");
        let key = file_key(&p);
        assert!(key.starts_with("file:"));
        assert_eq!(file_key_path(&key), Some(p));
        assert_eq!(file_key_path("https://x/y.jpg"), None);
    }
}
