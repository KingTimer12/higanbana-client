//! Navegação do leitor: virar página, capítulo, modo, faixa e divisão de arquivos.
//! Sem IO: quem chama grava preferências/progresso e busca o que `wants` pedir.

use higanbana_core::reader::api::OpenResult;
use higanbana_core::reader::logic::{build_spreads, expand_units, spread_of, Direction, ReadMode, Unit};

use super::state::{open_key, page_key, Opened, ReaderUi, PRELOAD};

/// Resultado de virar a página.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// Mudou página, capítulo ou o cartão de fim.
    Moved,
    /// Na faixa: rolar uma tela (sinal = sentido de leitura).
    Scroll(f32),
    None,
}

/// Página a buscar: unidade, posição no capítulo e índice no arquivo.
#[derive(Debug, Clone)]
pub struct PageWant {
    pub unit: Unit,
    pub page: usize,
    pub index: usize,
    pub key: String,
}

#[derive(Debug, Default)]
pub struct Wants {
    pub open: Vec<Unit>,
    pub pages: Vec<PageWant>,
}

/// Resumo de um arquivo dividido em capítulos.
#[derive(Debug, Clone)]
pub struct Summary {
    pub path: String,
    pub index: usize,
    pub count: usize,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Save {
    pub key: String,
    pub page: usize,
    pub total: usize,
    pub summary: Option<Summary>,
}

pub const MODES: [ReadMode; 4] = [ReadMode::Single, ReadMode::Double, ReadMode::Long, ReadMode::Wide];

impl ReaderUi {
    pub fn spreads(&self) -> Vec<Vec<usize>> {
        let Some(u) = self.unit() else { return Vec::new() };
        build_spreads(self.total(), |pg| self.ratios.get(&page_key(&u, pg)).copied().unwrap_or(0.) > 1.)
    }

    pub fn spread(&self) -> Vec<usize> {
        let s = self.spreads();
        s.get(spread_of(&s, self.page())).cloned().unwrap_or_default()
    }

    pub fn go_chapter(&mut self, i: usize, pg: Option<usize>) {
        let Some(u) = self.chapters.get(i).cloned() else { return };
        self.cur_key.clone_from(&u.key);
        self.page = pg;
        self.end = false;
        self.panel = None;
        self.strip_keep.clear();
        self.strip_wants.clear();
        self.strip = vec![u.key.clone()];
        if self.is_strip() {
            self.pending_scroll = Some((u.key, pg.unwrap_or(0)));
        }
        self.settle();
    }

    /// Capítulo `i` onde parou.
    pub fn resume_chapter(&mut self, i: usize) {
        let pg = self.resume_at(self.chapters.get(i));
        self.go_chapter(i, pg);
    }

    /// Resolve a página "-1" (última) quando o capítulo já tem páginas.
    fn settle(&mut self) {
        let total = self.total();
        if self.page.is_none() && total > 0 {
            self.page = Some(total - 1);
        }
    }

    pub fn next(&mut self) -> Step {
        if self.is_strip() {
            return Step::Scroll(1.0);
        }
        if self.end {
            if !self.has_next() {
                return Step::None;
            }
            self.go_chapter(self.chapter() + 1, Some(0));
            return Step::Moved;
        }
        let total = self.total();
        let target = if self.prefs.mode == ReadMode::Double {
            let s = self.spreads();
            s.get(spread_of(&s, self.page()) + 1).and_then(|x| x.first().copied()).unwrap_or(total)
        } else {
            self.page() + 1
        };
        if target < total {
            self.page = Some(target);
        } else if self.settings.seam && self.has_next() {
            self.go_chapter(self.chapter() + 1, Some(0));
        } else {
            self.end = true;
        }
        Step::Moved
    }

    pub fn prev(&mut self) -> Step {
        if self.is_strip() {
            return Step::Scroll(-1.0);
        }
        if self.end {
            self.end = false;
            return Step::Moved;
        }
        let target = if self.prefs.mode == ReadMode::Double {
            let s = self.spreads();
            spread_of(&s, self.page()).checked_sub(1).and_then(|i| s.get(i)).and_then(|x| x.first().copied())
        } else {
            self.page().checked_sub(1)
        };
        match target {
            Some(t) => self.page = Some(t),
            None if self.chapter() > 0 => self.go_chapter(self.chapter() - 1, None),
            None => return Step::None,
        }
        Step::Moved
    }

    /// Vai à página `pg` do capítulo atual (painel de páginas).
    pub fn go_page(&mut self, pg: usize) {
        self.panel = None;
        self.end = false;
        if self.is_strip() {
            self.strip = vec![self.cur_key.clone()];
            self.pending_scroll = Some((self.cur_key.clone(), pg));
        } else {
            self.page = Some(pg);
        }
    }

    pub fn set_mode(&mut self, m: ReadMode) {
        self.prefs.mode = m;
        self.strip_keep.clear();
        self.strip_wants.clear();
        self.strip = vec![self.cur_key.clone()];
        self.end = false;
        if self.is_strip() {
            self.pending_scroll = Some((self.cur_key.clone(), self.page()));
        }
    }

    pub fn cycle_mode(&mut self) {
        let i = MODES.iter().position(|m| *m == self.prefs.mode).unwrap_or(0);
        self.set_mode(MODES[(i + 1) % MODES.len()]);
    }

    pub fn toggle_direction(&mut self) {
        self.prefs.direction = if self.prefs.direction == Direction::Rtl { Direction::Ltr } else { Direction::Rtl };
    }

    /// Resposta de abrir um arquivo (chave de `open_key`). Chave desconhecida é ignorada.
    pub fn on_opened(&mut self, key: &str, r: Result<OpenResult, String>) {
        self.loading.remove(key);
        let Some(u) = self.chapters.iter().find(|u| open_key(u) == key).cloned() else { return };
        match r {
            Ok(result) => {
                let parts = if u.inner.is_none() && u.group.is_none() { expand_units(&u.path, &u.file, &result) } else { None };
                self.opened.insert(key.to_owned(), Opened::Ok(result));
                if let Some(parts) = parts {
                    self.expanded.insert(u.path.clone(), parts);
                    self.refresh_chapters();
                    self.split_current(&u.path);
                }
            }
            Err(e) => {
                self.opened.insert(key.to_owned(), Opened::Error(e));
            }
        }
        self.settle();
    }

    /// Esquece o erro do capítulo `ch` (item da faixa) para pedir de novo.
    pub fn retry_chapter(&mut self, ch: usize) {
        if let Some(u) = self.chapters.get(ch) {
            self.opened.remove(&open_key(u));
        }
    }

    /// Esquece o erro do capítulo atual para tentar de novo.
    pub fn retry_open(&mut self) {
        if let Some(u) = self.unit() {
            self.opened.remove(&open_key(&u));
        }
    }

    /// Arquivo que se dividiu em capítulos: troca a unidade inteira pelo capítulo certo.
    fn split_current(&mut self, path: &str) {
        let Some(parts) = self.expanded.get(path).cloned() else { return };
        for k in self.strip.iter_mut() {
            if k == path {
                k.clone_from(&parts[0].key);
            }
        }
        if self.cur_key != path {
            return;
        }
        let (pick, pg) = if self.page.is_none() {
            (parts.last().expect("divisão tem partes").clone(), None)
        } else if let Some(unit) = self.progress.get(path).and_then(|p| p.unit.clone()) {
            let at = parts.iter().position(|u| u.key == unit).unwrap_or(0);
            let done = self.progress.get(&parts[at].key).is_some_and(|p| p.read);
            let pick = parts[if done && at + 1 < parts.len() { at + 1 } else { at }].clone();
            let pg = self.resume_at(Some(&pick));
            (pick, pg)
        } else {
            (parts[0].clone(), Some(0))
        };
        for k in self.strip.iter_mut() {
            if *k == parts[0].key || k == path {
                k.clone_from(&pick.key);
            }
        }
        self.cur_key.clone_from(&pick.key);
        self.page = pg;
        if self.is_strip() {
            self.pending_scroll = Some((pick.key, pg.unwrap_or(0)));
        }
    }

    /// O que falta abrir/baixar para a posição atual (modos de página).
    pub fn wants(&self) -> Wants {
        let mut w = Wants::default();
        let Some(u) = self.unit() else { return w };
        let want_open = |w: &mut Wants, u: &Unit| {
            let k = open_key(u);
            if !self.opened.contains_key(&k) && !self.loading.contains(&k) {
                w.open.push(u.clone());
            }
        };
        want_open(&mut w, &u);
        let total = self.total();
        if total == 0 || self.is_strip() {
            return w;
        }
        let pg = self.page();
        let list = self.list();
        for k in 0..=PRELOAD {
            if let Some(p) = list.get(pg + k) {
                w.pages.push(PageWant { unit: u.clone(), page: pg + k, index: p.index, key: page_key(&u, pg + k) });
            }
        }
        if pg + PRELOAD >= total
            && let Some(next) = self.chapters.get(self.chapter() + 1)
        {
            want_open(&mut w, next);
        }
        w
    }

    /// Marca como pedido (não pede de novo até responder).
    pub fn mark_loading(&mut self, u: &Unit) {
        self.loading.insert(open_key(u));
    }

    /// Onde parou, para gravar (na página dupla, a mais adiantada do par).
    pub fn progress_to_save(&self) -> Option<Save> {
        let u = self.unit()?;
        let total = self.total();
        if total == 0 {
            return None;
        }
        let pg = self.page();
        let last = if self.prefs.mode == ReadMode::Double { self.spread().last().copied().unwrap_or(pg) } else { pg };
        let summary = self.expanded.get(&u.path).map(|parts| Summary {
            path: u.path.clone(),
            index: parts.iter().position(|p| p.key == u.key).unwrap_or(0),
            count: parts.len(),
            key: u.key.clone(),
            name: u.name.clone(),
        });
        Some(Save { key: u.key, page: last.min(total - 1), total, summary })
    }
}
