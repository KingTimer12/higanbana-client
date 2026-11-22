//! Carga do leitor: abre capítulos e baixa páginas num cache próprio, preso a uma
//! janela em volta da página atual (fora dela, a página é solta).

use std::collections::{HashMap, HashSet};

use higanbana_core::reader::PageRef;
use higanbana_gpui::{ImageData, ImageSource};

use super::nav::PageWant;
use super::state::{open_key, page_key, ReaderUi, PRELOAD};
use crate::image_cache::decode;
use crate::state::{AppState, Img};

/// Teto do lado maior de uma página decodificada.
const PAGE_MAX: u32 = 2048;
/// Páginas mantidas antes da atual (voltar sem baixar de novo).
const KEEP_BEHIND: usize = 2;

#[derive(Default)]
pub struct PageCache {
    map: HashMap<String, Img>,
}

impl PageCache {
    pub fn get(&self, key: &str) -> Option<&Img> {
        self.map.get(key)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn forget(&mut self, key: &str) {
        self.map.remove(key);
    }

    /// Solta tudo que não está em `keep`.
    pub fn retain_window(&mut self, keep: &HashSet<String>) {
        self.map.retain(|k, _| keep.contains(k));
    }
}

impl ReaderUi {
    /// Chaves que podem ficar no cache agora; `None` = não podar (faixa sem janela ainda).
    fn window(&self) -> Option<HashSet<String>> {
        if self.is_strip() {
            return (!self.strip_keep.is_empty()).then(|| self.strip_keep.clone());
        }
        let u = self.unit()?;
        let pg = self.page();
        Some((pg.saturating_sub(KEEP_BEHIND)..=pg + PRELOAD).map(|p| page_key(&u, p)).collect())
    }

    /// Poda o cache e devolve as páginas desejadas que ainda não foram pedidas (marcadas como carregando).
    pub fn request_pages(&mut self) -> Vec<PageWant> {
        if let Some(keep) = self.window() {
            self.pages.retain_window(&keep);
            // Na faixa a proporção define a altura de cada página: fica (um f32 por página).
            if !self.is_strip() {
                self.ratios.retain(|k, _| keep.contains(k));
            }
        }
        let mut wants = self.wants().pages;
        wants.extend(std::mem::take(&mut self.strip_wants));
        wants.retain(|w| self.pages.get(&w.key).is_none());
        for w in &wants {
            self.pages.map.insert(w.key.clone(), Img::Loading);
        }
        wants
    }

    /// Esquece a página (falhou) para pedir de novo.
    pub fn retry_page(&mut self, key: &str) {
        self.pages.forget(key);
    }

    fn on_page(&mut self, key: &str, r: Option<(ImageSource, f32)>) {
        // Só entra o que ainda está sendo esperado: a janela pode ter andado.
        if !matches!(self.pages.get(key), Some(Img::Loading)) {
            return;
        }
        match r {
            Some((img, ratio)) => {
                self.pages.map.insert(key.to_owned(), Img::Ready(img));
                self.ratios.insert(key.to_owned(), ratio);
            }
            None => {
                self.pages.map.insert(key.to_owned(), Img::Failed);
            }
        }
    }
}

/// Entrega uma página baixada ao leitor, se ele ainda está aberto.
pub fn deliver_page(slot: &mut Option<ReaderUi>, key: &str, r: Option<(ImageSource, f32)>) {
    if let Some(reader) = slot.as_mut() {
        reader.on_page(key, r);
    }
}

impl AppState {
    /// Depois de mudar a posição: pede o que falta, poda o cache e grava onde parou.
    pub fn pump_reader(&mut self) {
        let Some(r) = self.reader.as_mut() else { return };
        let mut opens = r.wants().open;
        for u in r.strip_opens() {
            if !opens.iter().any(|o| o.key == u.key) {
                opens.push(u);
            }
        }
        for u in &opens {
            r.mark_loading(u);
        }
        let pages = r.request_pages();
        self.save_reader_progress();
        for u in opens {
            let (core, key) = (self.core.clone(), open_key(&u));
            let fut = async move { core.reader().open(&u.path, u.file.size, u.inner.as_deref()).await };
            self.run(fut, move |s, res, _| {
                if let Some(r) = s.reader.as_mut() {
                    r.on_opened(&key, res);
                }
                s.pump_reader();
            });
        }
        for w in pages {
            let core = self.core.clone();
            let page = PageRef { size: w.unit.file.size, index: w.index, inner: w.unit.inner.clone(), path: w.unit.path.clone() };
            let fut = async move {
                let bytes = match core.reader().page(&page).await {
                    Ok((b, _)) => b,
                    Err(e) => {
                        tracing::warn!(pagina = %page.path, erro = %e, "leitor: página não carregou");
                        return None;
                    }
                };
                tokio::task::spawn_blocking(move || decode(&bytes, PAGE_MAX, PAGE_MAX)).await.ok().flatten()
            };
            let key = w.key;
            self.run(fut, move |s, res, _| {
                let page = res.map(|(d, _)| {
                    let ratio = if d.height > 0 { d.width as f32 / d.height as f32 } else { 0.7 };
                    (ImageData::from_rgba8(d.width, d.height, d.rgba), ratio)
                });
                deliver_page(&mut s.reader, &key, page);
            });
        }
    }

    /// Grava a posição atual (só quando mudou).
    fn save_reader_progress(&mut self) {
        let Some(r) = self.reader.as_mut() else { return };
        let Some(s) = r.progress_to_save() else { return };
        let mark = (s.key.clone(), s.page);
        if r.last_saved.as_ref() == Some(&mark) {
            return;
        }
        r.last_saved = Some(mark);
        let store = &self.core.reader().store;
        store.set_progress(&s.key, s.page, s.total);
        if let Some(sum) = &s.summary {
            let read = store.data().progress.get(&s.key).is_some_and(|p| p.read);
            store.set_summary(&sum.path, sum.index, sum.count, read, &sum.key, &sum.name);
        }
        r.progress = store.data().progress;
    }
}
