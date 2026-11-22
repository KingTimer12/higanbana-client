//! Faixa (long/wide strip): capítulos em sequência, emenda do próximo, posição
//! pelo item no topo e janela de páginas em volta do visível.

use std::ops::Range;

use higanbana_core::reader::logic::{Direction, ReadMode, Unit};

use super::nav::PageWant;
use super::state::{open_key, page_key, Opened, ReaderUi};

/// Itens a mais de cada lado do visível que ficam baixados.
pub const AROUND: usize = 6;

#[derive(Debug, Clone)]
pub enum StripKind {
    Page { unit: Unit, page: usize, index: usize },
    Loading,
    End(usize),
    Seam(usize),
    Failed(usize, String),
}

#[derive(Debug, Clone)]
pub struct StripItem {
    /// (capítulo, página) que o item representa (posição de leitura).
    pub key: (String, usize),
    pub kind: StripKind,
}

impl ReaderUi {
    pub fn strip_items(&self) -> Vec<StripItem> {
        let mut items = Vec::new();
        for key in &self.strip {
            let Some(ch) = self.chapters.iter().position(|u| &u.key == key) else { continue };
            let u = &self.chapters[ch];
            if let Some(Opened::Error(e)) = self.opened.get(&open_key(u)) {
                items.push(StripItem { key: (u.key.clone(), 0), kind: StripKind::Failed(ch, e.clone()) });
                continue;
            }
            let list = self.list_of(u).unwrap_or_default();
            for (pg, info) in list.iter().enumerate() {
                items.push(StripItem { key: (u.key.clone(), pg), kind: StripKind::Page { unit: u.clone(), page: pg, index: info.index } });
            }
            if list.is_empty() {
                items.push(StripItem { key: (u.key.clone(), 0), kind: StripKind::Loading });
            }
            let last = (u.key.clone(), list.len().saturating_sub(1));
            if !self.settings.seam || ch + 1 >= self.chapters.len() {
                items.push(StripItem { key: last, kind: StripKind::End(ch) });
            } else if self.strip.contains(&self.chapters[ch + 1].key) {
                items.push(StripItem { key: last, kind: StripKind::Seam(ch) });
            }
        }
        // Na wide strip da direita para a esquerda, a primeira página fica à direita.
        if self.prefs.mode == ReadMode::Wide && self.prefs.direction == Direction::Rtl {
            items.reverse();
        }
        items
    }

    /// Item `ix` no topo (ou no centro): vira a posição atual. Devolve se mudou.
    pub fn on_strip_top(&mut self, items: &[StripItem], ix: usize) -> bool {
        let Some(item) = items.get(ix) else { return false };
        let (key, pg) = item.key.clone();
        if key == self.cur_key && Some(pg) == self.page {
            return false;
        }
        self.cur_key = key;
        self.page = Some(pg);
        true
    }

    /// Emenda o próximo capítulo (com a opção ligada e o último da faixa já aberto).
    pub fn extend_strip(&mut self) -> bool {
        if !self.settings.seam {
            return false;
        }
        let Some(last) = self.strip.last().and_then(|k| self.chapters.iter().position(|u| &u.key == k)) else { return false };
        let Some(next) = self.chapters.get(last + 1) else { return false };
        if self.list_of(&self.chapters[last]).is_none() || self.strip.contains(&next.key) {
            return false;
        }
        self.strip.push(next.key.clone());
        true
    }

    /// Capítulos da faixa ainda não abertos.
    pub fn strip_opens(&self) -> Vec<Unit> {
        self.strip
            .iter()
            .filter_map(|k| self.chapters.iter().find(|u| &u.key == k))
            .filter(|u| {
                let k = open_key(u);
                !self.opened.contains_key(&k) && !self.loading.contains(&k)
            })
            .cloned()
            .collect()
    }

    /// Páginas a baixar e a manter: os itens visíveis e `AROUND` de cada lado.
    pub fn set_strip_window(&mut self, items: &[StripItem], visible: Range<usize>) {
        let from = visible.start.saturating_sub(AROUND);
        let to = (visible.end + AROUND).min(items.len());
        self.strip_keep.clear();
        self.strip_wants.clear();
        for item in items.get(from..to).unwrap_or(&[]) {
            if let StripKind::Page { unit, page, index } = &item.kind {
                let key = page_key(unit, *page);
                self.strip_keep.insert(key.clone());
                self.strip_wants.push(PageWant { unit: unit.clone(), page: *page, index: *index, key });
            }
        }
    }
}
