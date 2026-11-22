//! Estado do leitor aberto (guardado em `Higanbana.reader`, solto ao fechar).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use higanbana_core::library::join_path;
use higanbana_core::reader::api::{OpenResult, PageInfo};
use higanbana_core::reader::logic::{base_unit, comic_files, pages_of_group, sort_pages, ReadMode, Unit};
use higanbana_core::reader::store::{ReadProgress, ReaderData, ReaderSettings, TitlePrefs};
use higanbana_gpui::TimerId;
use higanbana_gpui::TextInputState;

use super::ReadTarget;

/// Páginas carregadas antes de chegar nelas (modos de página).
pub const PRELOAD: usize = 3;

pub enum Opened {
    Ok(OpenResult),
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    Index,
    Pages,
    Options,
}

/// Um zip aberto: o arquivo, ou o `.cbz` guardado dentro dele.
pub fn open_key(u: &Unit) -> String {
    match &u.inner {
        Some(i) => format!("{}::{i}", u.path),
        None => u.path.clone(),
    }
}

pub fn page_key(u: &Unit, pg: usize) -> String {
    format!("page:{}:{pg}", u.key)
}

pub struct ReaderUi {
    pub target: ReadTarget,
    /// Arquivos com vários capítulos dentro viram várias unidades quando abertos.
    pub(crate) expanded: HashMap<String, Vec<Unit>>,
    /// Capítulos em ordem, refeitos só quando `expanded` muda.
    pub chapters: Rc<[Unit]>,
    pub(crate) opened: HashMap<String, Opened>,
    pub(crate) loading: HashSet<String>,
    pub cur_key: String,
    /// Página no capítulo atual; `None` = ir para a última quando carregar.
    pub page: Option<usize>,
    pub prefs: TitlePrefs,
    pub settings: ReaderSettings,
    /// Progresso salvo (cópia do núcleo, atualizada a cada gravação).
    pub progress: HashMap<String, ReadProgress>,
    /// Proporção (largura/altura) das páginas já baixadas.
    pub ratios: HashMap<String, f32>,
    /// Capítulos na faixa (long/wide strip), em sequência.
    pub strip: Vec<String>,
    pub pending_scroll: Option<(String, usize)>,
    pub end: bool,
    pub panel: Option<Panel>,
    /// Mouse mexeu há pouco (mostra o progresso escondido).
    pub stirred: bool,
    pub stir_timer: Option<TimerId>,
    pub filter: TextInputState,
    /// O filtro do índice tem o foco (as teclas do leitor ficam quietas).
    pub filter_focused: bool,
    /// Páginas baixadas (só a janela em volta da atual).
    pub pages: super::load::PageCache,
    /// Na faixa: chaves que podem ficar no cache e páginas pedidas pela tela (Task da faixa).
    pub strip_keep: HashSet<String>,
    pub strip_wants: Vec<super::nav::PageWant>,
    /// Faixa: deslocamento e tamanho da vista no último quadro (para rolar por tecla).
    pub strip_scroll: (f32, f32),
    /// Última posição gravada (não regrava a cada quadro).
    pub last_saved: Option<(String, usize)>,
}

impl ReaderUi {
    pub fn new(target: ReadTarget, data: &ReaderData) -> Self {
        let start = join_path(&target.dir, &target.file.name);
        let mut r = ReaderUi {
            prefs: data.prefs.get(&target.title_path).copied().unwrap_or_default(),
            settings: data.settings,
            progress: data.progress.clone(),
            target,
            expanded: HashMap::new(),
            chapters: Rc::from([]),
            opened: HashMap::new(),
            loading: HashSet::new(),
            cur_key: start.clone(),
            page: Some(0),
            ratios: HashMap::new(),
            strip: vec![start.clone()],
            pending_scroll: None,
            end: false,
            panel: None,
            stirred: false,
            stir_timer: None,
            filter: TextInputState::new().placeholder("Buscar capítulo ou volume"),
            filter_focused: false,
            pages: Default::default(),
            strip_keep: HashSet::new(),
            strip_wants: Vec::new(),
            strip_scroll: (0.0, 0.0),
            last_saved: None,
        };
        r.refresh_chapters();
        let i = r.index_of(&start).unwrap_or(0);
        let resume = r.resume_at(r.chapters.get(i));
        r.go_chapter(i, resume);
        r
    }

    pub(crate) fn refresh_chapters(&mut self) {
        let dir = &self.target.dir;
        self.chapters = comic_files(&self.target.files)
            .into_iter()
            .flat_map(|f| {
                let path = join_path(dir, &f.name);
                self.expanded.get(&path).cloned().unwrap_or_else(|| vec![base_unit(&path, &f)])
            })
            .collect();
    }

    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.chapters.iter().position(|u| u.key == key)
    }

    pub fn chapter(&self) -> usize {
        self.index_of(&self.cur_key).unwrap_or(0)
    }

    pub fn unit(&self) -> Option<Unit> {
        self.chapters.get(self.chapter()).cloned()
    }

    /// Arquivo inteiro que virou vários capítulos: as páginas são dos capítulos, não dele.
    fn split_away(&self, u: &Unit) -> bool {
        u.inner.is_none() && u.group.is_none() && self.expanded.contains_key(&u.path)
    }

    pub fn list_of(&self, u: &Unit) -> Option<Vec<PageInfo>> {
        let Some(Opened::Ok(r)) = self.opened.get(&open_key(u)) else { return None };
        if self.split_away(u) {
            return None;
        }
        Some(sort_pages(&pages_of_group(&r.pages, u.group.as_deref())))
    }

    pub fn list(&self) -> Vec<PageInfo> {
        self.unit().and_then(|u| self.list_of(&u)).unwrap_or_default()
    }

    /// Erro ao abrir o capítulo atual.
    pub fn open_error(&self) -> Option<&str> {
        let u = self.chapters.get(self.chapter())?;
        match self.opened.get(&open_key(u)) {
            Some(Opened::Error(e)) => Some(e),
            _ => None,
        }
    }

    pub fn total(&self) -> usize {
        let Some(u) = self.chapters.get(self.chapter()) else { return 0 };
        let Some(Opened::Ok(r)) = self.opened.get(&open_key(u)) else { return 0 };
        if self.split_away(u) {
            return 0;
        }
        if u.group.is_none() { r.pages.len() } else { pages_of_group(&r.pages, u.group.as_deref()).len() }
    }

    pub fn page(&self) -> usize {
        self.page.unwrap_or(0)
    }

    pub fn is_strip(&self) -> bool {
        matches!(self.prefs.mode, ReadMode::Long | ReadMode::Wide)
    }

    /// Onde abrir um capítulo: a página salva, se ainda não terminou.
    pub fn resume_at(&self, u: Option<&Unit>) -> Option<usize> {
        let p = u.and_then(|u| self.progress.get(&u.key));
        Some(p.filter(|p| !p.read && p.unit.is_none()).map_or(0, |p| p.page))
    }

    /// Arquivo aberto sem nenhuma página (nada a mostrar além do aviso).
    pub fn empty_chapter(&self) -> bool {
        let Some(u) = self.chapters.get(self.chapter()) else { return false };
        matches!(self.opened.get(&open_key(u)), Some(Opened::Ok(_))) && !self.split_away(u) && self.total() == 0
    }

    pub fn has_next(&self) -> bool {
        self.chapter() + 1 < self.chapters.len()
    }

    pub fn toggle(&mut self, p: Panel) {
        self.panel = if self.panel == Some(p) { None } else { Some(p) };
    }
}
