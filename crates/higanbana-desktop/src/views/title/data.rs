//! Modelo da tela de título, montado a cada render a partir das listagens:
//!   <Título>/[ANO] <Temporada>/<Release [tags]>/arquivos
//! Temporadas = subpastas com ano; releases = subpastas da temporada. Anime com
//! a maioria das subpastas datadas é franquia (cada subpasta é uma obra).

use std::sync::Arc;

use higanbana_core::anilist::MediaType;
use higanbana_core::library::{base_name, folder_title, folder_year, is_extra_folder, is_franchise, join_path, natural_cmp, resolution_rank, strip_numbered, version_label, VersionLabel, LOOSE_FILES_VERSION};
use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::logic::video_files;

use crate::state::{AppState, Listing};

pub type Files = Option<Result<Arc<[FileEntry]>, String>>;

pub fn of(l: Listing) -> Files {
    match l {
        Listing::Loading => None,
        Listing::Ready(f) => Some(Ok(f)),
        Listing::Failed(e) => Some(Err(e)),
    }
}

fn files(l: &Files) -> Option<&[FileEntry]> {
    match l {
        Some(Ok(f)) => Some(f),
        _ => None,
    }
}

fn dirs_of(l: &Files) -> Vec<FileEntry> {
    let mut v: Vec<FileEntry> = files(l).map(|f| f.iter().filter(|f| f.is_directory).cloned().collect()).unwrap_or_default();
    v.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    v
}

/// Subpastas da obra separadas em versões (releases) e extras.
pub fn split_work_dirs(dirs: Vec<FileEntry>) -> (Vec<FileEntry>, Vec<FileEntry>) {
    dirs.into_iter().partition(|d| !is_extra_folder(&d.name))
}

/// Versão aberta quando o caminho não diz qual: a lembrada, se ainda existe; senão a de maior resolução.
pub fn default_version(versions: &[FileEntry], remembered: Option<&str>, work_title: &str) -> Option<String> {
    if let Some(r) = remembered.filter(|r| versions.iter().any(|v| v.name == *r)) {
        return Some(r.to_owned());
    }
    let rank = |v: &FileEntry| resolution_rank(&version_label(&v.name, work_title));
    versions.iter().fold(None::<&FileEntry>, |best, v| match best {
        Some(b) if rank(b) >= rank(v) => Some(b),
        _ => Some(v),
    })
    .map(|v| v.name.clone())
}

/// O detalhe `detail_id` é da obra aberta: casa com o resumo atual.
pub fn is_open_work(detail_id: i64, detail_for: Option<i64>, current: Option<i64>) -> bool {
    detail_for == Some(detail_id) && current == Some(detail_id)
}

/// Rótulo do cartão de uma versão; a sintética de arquivos soltos tem nome fixo.
pub fn label_of(folder: &str, work_title: &str) -> VersionLabel {
    if folder == LOOSE_FILES_VERSION { VersionLabel { name: LOOSE_FILES_VERSION.to_owned(), ..VersionLabel::default() } } else { version_label(folder, work_title) }
}

pub struct Model {
    pub title_path: String,
    pub rest: Vec<String>,
    pub media: MediaType,
    pub title: Files,
    pub title_dirs: Vec<FileEntry>,
    pub franchise: bool,
    pub season: Files,
    pub work_split: (Vec<FileEntry>, Vec<FileEntry>),
}

impl Model {
    /// Pede as listagens que faltam (título, depois temporada/obra).
    pub fn load(s: &mut AppState, title_path: &str, rest: &[String], media: MediaType) -> Model {
        let title = of(s.listing(title_path));
        let title_dirs = dirs_of(&title);
        let franchise = media == MediaType::Anime && is_franchise(&title_dirs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>());
        let mut m = Model { title_path: title_path.to_owned(), rest: rest.to_vec(), media, title, title_dirs, franchise, season: None, work_split: Default::default() };
        if m.title.is_some()
            && m.season_name().is_some()
        {
            m.season = of(s.listing(&m.season_path()));
        }
        let dirs = if m.season_name().is_some() { dirs_of(&m.season) } else { m.title_dirs.iter().filter(|d| folder_year(&d.name).is_none()).cloned().collect() };
        m.work_split = split_work_dirs(dirs);
        m
    }

    pub fn seasons(&self) -> Vec<FileEntry> {
        self.title_dirs.iter().filter(|d| folder_year(&d.name).is_some()).cloned().collect()
    }

    /// Na franquia sem obra escolhida, a tela é a da franquia.
    pub fn shows_franchise(&self) -> bool {
        self.franchise && self.season_name().is_none()
    }

    /// Temporada (ou obra, na franquia) aberta.
    pub fn season_name(&self) -> Option<String> {
        if self.franchise {
            let wanted = self.rest.first()?;
            return self.title_dirs.iter().find(|d| &d.name == wanted).map(|d| d.name.clone());
        }
        let seasons = self.seasons();
        let first = seasons.first()?.name.clone();
        Some(self.rest.first().filter(|w| seasons.iter().any(|s| &&s.name == w)).cloned().unwrap_or(first))
    }

    pub fn season_path(&self) -> String {
        self.season_name().map_or_else(|| self.title_path.clone(), |s| join_path(&self.title_path, &s))
    }

    pub fn rest_after_season(&self) -> Vec<String> {
        match (self.season_name(), self.rest.first()) {
            (Some(s), Some(r)) if &s == r => self.rest[1..].to_vec(),
            _ => self.rest.clone(),
        }
    }

    fn work_files(&self) -> &Files {
        if self.season_name().is_some() { &self.season } else { &self.title }
    }

    pub fn lookup_name(&self) -> String {
        match self.season_name() {
            // Obra de franquia numerada (`01 Nisemonogatari`): o número não faz parte do título.
            Some(s) if self.franchise => strip_numbered(&s).to_owned(),
            Some(s) => s,
            None => base_name(&self.title_path).to_owned(),
        }
    }

    pub fn work_title(&self) -> String {
        folder_title(&self.lookup_name())
    }

    /// Versões; vídeos soltos ao lado de pastas de versão viram a versão sintética.
    pub fn releases(&self) -> Vec<FileEntry> {
        let mut releases = self.work_split.0.clone();
        let loose = files(self.work_files()).is_some_and(|f| !video_files(f).is_empty());
        if loose && !releases.is_empty() {
            releases.push(FileEntry { name: LOOSE_FILES_VERSION.to_owned(), is_directory: true, size: 0, modified: String::new(), extension: None });
        }
        releases
    }

    /// Pasta extra aberta pelo caminho (`<obra>/Extras/...`).
    pub fn extra_name(&self) -> Option<String> {
        let rest = self.rest_after_season();
        let wanted = rest.first()?;
        self.work_split.1.iter().find(|e| &e.name == wanted).map(|e| e.name.clone())
    }

    /// Versão aberta: a do caminho; senão a lembrada ou a de maior resolução.
    pub fn release_name(&self, remembered: Option<&str>) -> Option<String> {
        let releases = self.releases();
        let rest = self.rest_after_season();
        if let Some(r) = rest.first().filter(|w| releases.iter().any(|r| &&r.name == w)) {
            return Some(r.clone());
        }
        default_version(&releases, remembered, &self.work_title())
    }

    /// Pasta dos arquivos, ou `None` enquanto as listagens de cima não chegaram.
    pub fn files_path(&self, remembered: Option<&str>) -> Option<String> {
        self.title.as_ref()?;
        if self.season_name().is_some() && self.season.is_none() {
            return None;
        }
        let season_path = self.season_path();
        let rest = self.rest_after_season();
        if self.extra_name().is_some() {
            return Some(std::iter::once(season_path).chain(rest).collect::<Vec<_>>().join("/"));
        }
        let release = match self.release_name(remembered) {
            None => return Some(season_path),
            Some(r) if r == LOOSE_FILES_VERSION => return Some(season_path),
            Some(r) => r,
        };
        let deeper: Vec<String> = if rest.first() == Some(&release) { rest[1..].to_vec() } else { Vec::new() };
        Some(std::iter::once(season_path).chain(std::iter::once(release)).chain(deeper).collect::<Vec<_>>().join("/"))
    }
}

/// Arquivos que a tela mostra: na raiz da obra as subpastas ficam de fora
/// (versões viram cartões, extras têm seção própria).
pub fn shown_files(list: &[FileEntry], at_root: bool) -> Vec<FileEntry> {
    if at_root { list.iter().filter(|f| !f.is_directory).cloned().collect() } else { list.to_vec() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> FileEntry {
        FileEntry { name: name.to_owned(), is_directory: true, size: 0, modified: String::new(), extension: None }
    }

    fn smile() -> Vec<FileEntry> {
        vec![dir("[Aenianos] Smile Precure [1280x720][hi10p]"), dir("[DragsterPS] Glitter Force S01 e S02 [1080p] [Multi-Audio] [Multi-Subs]"), dir("Extras")]
    }

    #[test]
    fn split_work_dirs_separates_extras() {
        let (versions, extras) = split_work_dirs(smile());
        assert_eq!(versions.len(), 2);
        assert_eq!(extras.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Extras"]);
    }

    #[test]
    fn default_version_prefers_remembered() {
        let (versions, _) = split_work_dirs(smile());
        let aenianos = versions[0].name.clone();
        assert_eq!(default_version(&versions, Some(&aenianos), "Smile Precure!"), Some(aenianos));
    }

    #[test]
    fn default_version_falls_back_to_best_resolution() {
        let (versions, _) = split_work_dirs(smile());
        let glitter = versions[1].name.clone();
        assert_eq!(default_version(&versions, Some("sumiu"), "Smile Precure!"), Some(glitter.clone()));
        assert_eq!(default_version(&versions, None, "Smile Precure!"), Some(glitter));
        assert_eq!(default_version(&[], None, "Smile Precure!"), None);
    }

    #[test]
    fn detail_of_previous_work_is_ignored() {
        assert!(is_open_work(7, Some(7), Some(7)));
        assert!(!is_open_work(7, Some(7), None));
        assert!(!is_open_work(7, Some(7), Some(8)));
        assert!(!is_open_work(7, Some(8), Some(8)));
        assert!(!is_open_work(7, None, Some(7)));
    }

    fn model(dirs: &[&str], rest: &[&str]) -> Model {
        let title_dirs: Vec<FileEntry> = dirs.iter().map(|d| dir(d)).collect();
        let franchise = is_franchise(&title_dirs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>());
        let title: Arc<[FileEntry]> = title_dirs.clone().into();
        Model { title_path: "Animes/Letra P/Precure".into(), rest: rest.iter().map(|s| s.to_string()).collect(), media: MediaType::Anime, title: Some(Ok(title)), title_dirs, franchise, season: None, work_split: Default::default() }
    }

    #[test]
    fn franquia_sem_obra_mostra_franquia() {
        let m = model(&["[2004] Futari wa", "[2005] Max Heart", "[2010] Heartcatch"], &[]);
        assert!(m.shows_franchise());
        let m = model(&["[2004] Futari wa", "[2005] Max Heart", "[2010] Heartcatch"], &["[2005] Max Heart"]);
        assert!(!m.shows_franchise());
        assert_eq!(m.season_path(), "Animes/Letra P/Precure/[2005] Max Heart");
    }

    #[test]
    fn raiz_sem_pastas_na_lista() {
        let mut f = dir("ep01.mkv");
        f.is_directory = false;
        let list = vec![dir("Extras"), f];
        assert_eq!(shown_files(&list, true).len(), 1);
        assert_eq!(shown_files(&list, false).len(), 2);
    }
}
