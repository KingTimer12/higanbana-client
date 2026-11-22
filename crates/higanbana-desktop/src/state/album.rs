//! Álbum da pasta aberta (só um; solto ao sair da pasta) e a capa dele.

use std::rc::Rc;
use std::sync::Arc;

use base64::Engine as _;
use higanbana_core::music::album::Album;
use higanbana_core::nuvem::FileEntry;

use super::{AppState, Img};

pub enum AlbumLoad {
    Loading,
    Ready(Rc<Album>),
    /// Nada tocável: a pasta mostra a lista de arquivos.
    NotAlbum,
}

pub struct AlbumSlot {
    pub path: String,
    pub load: AlbumLoad,
    /// Aba "Arquivos" aberta.
    pub files_tab: bool,
}

/// Solta o álbum se a tela atual não é a pasta dele.
pub fn keep_album(slot: &mut Option<AlbumSlot>, path: &str) {
    if slot.as_ref().is_some_and(|s| s.path != path) {
        *slot = None;
    }
}

/// Chave da capa no cache de imagens.
pub fn cover_key(album_path: &str) -> String {
    format!("album-cover:{album_path}")
}

impl AppState {
    /// Álbum da pasta `path` (pede na primeira vez).
    pub fn album(&mut self, path: &str, files: &Arc<[FileEntry]>) -> &AlbumLoad {
        if self.album.as_ref().is_none_or(|s| s.path != path) {
            self.album = Some(AlbumSlot { path: path.to_owned(), load: AlbumLoad::Loading, files_tab: false });
            let (core, p, f) = (self.core.clone(), path.to_owned(), files.to_vec());
            let fut = {
                let p = p.clone();
                async move { core.cloud().load_album(&p, &f).await }
            };
            // Resposta de uma pasta que já foi deixada é descartada.
            self.run(fut, move |s, album, _| {
                if s.album.as_ref().is_some_and(|slot| slot.path == p) {
                    s.album_loaded(album);
                }
            });
        }
        &self.album.as_ref().expect("acabou de entrar").load
    }

    fn album_loaded(&mut self, album: Option<Album>) {
        let Some(slot) = self.album.as_mut() else { return };
        match album {
            Some(a) => {
                let a = Rc::new(a);
                slot.load = AlbumLoad::Ready(a.clone());
                self.show_album(&a);
            }
            None => slot.load = AlbumLoad::NotAlbum,
        }
    }

    /// Capa da pasta; sem ela, a capa embutida na primeira faixa.
    pub fn album_cover(&mut self, a: &Album) -> Img {
        let core = self.core.clone();
        let cover = a.cover_path.clone().zip(a.cover_size);
        let first = a.tracks.first().cloned();
        self.image_from(&cover_key(&a.path), async move {
            if let Some((path, size)) = cover
                && let Ok(b) = core.cloud().image(&path, size).await
            {
                return Some(b);
            }
            let t = first?;
            let meta = core.music().meta(&t.path, t.size, &t.ext, true).await.ok()?;
            let data = meta.cover?;
            base64::engine::general_purpose::STANDARD.decode(data.split_once(',')?.1).ok()
        })
    }
}
