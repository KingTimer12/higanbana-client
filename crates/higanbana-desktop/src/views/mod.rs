//! Telas do acervo.

pub mod catalog;
pub mod catalog_filters;
pub mod folder;
pub mod franchise;
pub mod home;
pub mod login;
pub mod shelf;
pub mod title;

use gpui_kit::{Context, Entity};
use higanbana_core::nuvem::FileEntry;

use crate::state::AppState;

/// Lista uma pasta da nuvem e entrega o resultado à tela.
pub fn load_listing<V: 'static>(
    state: &Entity<AppState>,
    path: String,
    cx: &mut Context<V>,
    set: impl FnOnce(&mut V, Result<Vec<FileEntry>, String>, &mut Context<V>) + 'static,
) {
    let core = state.read(cx).core.clone();
    let task = core.clone().spawn(async move { core.cloud().list(&path).await.map(|l| l.files).map_err(|e| e.to_string()) });
    cx.spawn(async move |this, cx| {
        let r = task.await.map_err(|e| e.to_string()).and_then(|r| r);
        let _ = this.update(cx, |v, cx| {
            set(v, r, cx);
            cx.notify();
        });
    })
    .detach();
}
