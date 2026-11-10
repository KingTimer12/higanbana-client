//! Página de um título. Estrutura esperada na nuvem:
//!   <Título>/[ANO] <Temporada>/<Release [tags]>/arquivos
//! Temporadas = subpastas com ano; releases = subpastas da temporada.
//! Cada temporada casa com sua própria entrada no AniList.
//! Anime com a maioria das subpastas datadas é franquia: sem obra escolhida
//! mostra a [`FranchiseView`]; com obra, `title_path/<obra>` vira a raiz da página.

use crate::interact::hover_layer;
use crate::interact::Button;
use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui_kit::{
    AnyElement, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Subscription,
    Window, assets::IconName, div, prelude::*, px, relative,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex,
    avatar::Avatar,
    button::ButtonVariants as _,
    link::Link,
    tab::{Tab, TabBar},
    tag::Tag,
};
use higanbana_core::{
    anilist::{
        MediaType, text,
        types::{MediaDetail, MediaSummary},
    },
    library::{
        LOOSE_FILES_VERSION, VersionLabel, base_name, folder_title, folder_year, is_extra_folder, is_franchise, join_path,
        natural_cmp, raw_path, release_tags, resolution_rank, version_label,
    },
    nuvem::FileEntry,
    player::logic::{episode_number, format_time, video_files},
    reader::logic::{chapter_label, comic_files, label_text},
    tmdb::{
        WorkEpisodes, work_key,
        matching::work_query,
    },
};

use crate::{
    components::{
        breadcrumbs, columns, content_width, cover, file_list, file_paths, hex_color, option_button, option_group,
        remote_image, rounded_image, version_card, FileAction,
    },
    music::album::{AlbumEvent, AlbumView, looks_like_album},
    reader::{ReadTarget, progress_map},
    state::{AppState, DlStatus, state},
    theme::ProductColors as _,
    ui::{empty_state, truncate},
    video::PlayTarget,
    views::{episodes::episode_list, franchise::FranchiseView, load_listing},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Files,
    Cast,
    Staff,
    Related,
    Details,
}

type Listing = Option<Result<Vec<FileEntry>, String>>;

pub struct TitleView {
    state: Entity<AppState>,
    title_path: String,
    rest: Vec<String>,
    media_type: MediaType,
    title_listing: Listing,
    season_listing: Listing,
    files_listing: Listing,
    /// Caminho cuja listagem está em `files_listing` (ou sendo buscada).
    files_for: Option<String>,
    season_for: Option<String>,
    detail: Option<Option<MediaDetail>>,
    detail_for: Option<i64>,
    /// Chave (`tmdb::work_key`) dos episódios pedidos para a obra aberta.
    episodes_key: Option<String>,
    tab: Section,
    expanded: bool,
    album: Option<Entity<AlbumView>>,
    not_album: bool,
    franchise: Option<Entity<FranchiseView>>,
    /// Subpastas de `title_listing`, ordenadas (guardadas quando a listagem chega).
    title_dirs: Vec<FileEntry>,
    /// Anime cuja pasta tem a maioria das subpastas datadas (cada uma é uma obra).
    franchise_folder: bool,
    /// Subpastas da obra (versões, extras), refeitas quando uma listagem de cima chega.
    work_split: (Vec<FileEntry>, Vec<FileEntry>),
    _subs: Vec<Subscription>,
}

fn dirs(files: &Listing) -> Vec<FileEntry> {
    let mut v: Vec<FileEntry> = match files {
        Some(Ok(f)) => f.iter().filter(|f| f.is_directory).cloned().collect(),
        _ => Vec::new(),
    };
    v.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    v
}

fn files_of(l: &Listing) -> Option<&Vec<FileEntry>> {
    match l {
        Some(Ok(f)) => Some(f),
        _ => None,
    }
}

/// Subpastas da obra separadas em versões (releases) e extras.
pub(crate) fn split_work_dirs(dirs: Vec<FileEntry>) -> (Vec<FileEntry>, Vec<FileEntry>) {
    dirs.into_iter().partition(|d| !is_extra_folder(&d.name))
}

/// Versão aberta quando o caminho não diz qual: a lembrada, se ainda existe;
/// senão a de maior resolução (a primeira, no empate).
pub(crate) fn default_version(versions: &[FileEntry], remembered: Option<&str>, work_title: &str) -> Option<String> {
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

/// O detalhe `detail_id` é da obra aberta: foi o pedido por último e casa com o resumo atual.
pub(crate) fn is_open_work(detail_id: i64, detail_for: Option<i64>, current: Option<i64>) -> bool {
    detail_for == Some(detail_id) && current == Some(detail_id)
}

/// Rótulo do cartão de uma versão; a sintética de arquivos soltos tem nome fixo.
fn label_of(folder: &str, work_title: &str) -> VersionLabel {
    if folder == LOOSE_FILES_VERSION {
        VersionLabel { name: LOOSE_FILES_VERSION.to_owned(), ..VersionLabel::default() }
    } else {
        version_label(folder, work_title)
    }
}

impl TitleView {
    pub fn new(title_path: String, rest: Vec<String>, media_type: MediaType, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let subs = vec![cx.observe(&state, |v: &mut Self, _, cx| {
            v.sync(cx);
            cx.notify();
        })];
        load_listing(&state, title_path.clone(), cx, |v: &mut Self, r, cx| {
            v.title_listing = Some(r);
            v.title_dirs = dirs(&v.title_listing);
            v.franchise_folder = v.media_type == MediaType::Anime
                && is_franchise(&v.title_dirs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>());
            v.refresh_work_dirs();
            v.sync(cx);
        });
        Self {
            state,
            title_path,
            rest,
            media_type,
            title_listing: None,
            season_listing: None,
            files_listing: None,
            files_for: None,
            season_for: None,
            detail: None,
            detail_for: None,
            episodes_key: None,
            tab: Section::Files,
            expanded: false,
            album: None,
            not_album: false,
            franchise: None,
            title_dirs: Vec::new(),
            franchise_folder: false,
            work_split: (Vec::new(), Vec::new()),
            _subs: subs,
        }
    }

    fn seasons(&self) -> Vec<FileEntry> {
        self.title_dirs.iter().filter(|d| folder_year(&d.name).is_some()).cloned().collect()
    }

    fn is_franchise(&self) -> bool {
        self.franchise_folder
    }

    /// Na franquia sem obra escolhida, quem desenha a página é a [`FranchiseView`].
    fn shows_franchise(&self) -> bool {
        self.is_franchise() && self.season_name().is_none()
    }

    /// Temporada (ou obra, na franquia) aberta.
    fn season_name(&self) -> Option<String> {
        if self.is_franchise() {
            let wanted = self.rest.first()?;
            return self.title_dirs.iter().find(|d| &d.name == wanted).map(|d| d.name.clone());
        }
        let seasons = self.seasons();
        let first = seasons.first()?.name.clone();
        let wanted = self.rest.first();
        Some(wanted.filter(|w| seasons.iter().any(|s| &&s.name == w)).cloned().unwrap_or(first))
    }

    fn season_path(&self) -> String {
        self.season_name().map_or_else(|| self.title_path.clone(), |s| join_path(&self.title_path, &s))
    }

    fn rest_after_season(&self) -> Vec<String> {
        match (self.season_name(), self.rest.first()) {
            (Some(s), Some(r)) if &s == r => self.rest[1..].to_vec(),
            _ => self.rest.clone(),
        }
    }

    /// Listagem da pasta da obra (temporada, obra da franquia ou o próprio título).
    fn work_listing(&self) -> &Listing {
        if self.season_name().is_some() { &self.season_listing } else { &self.title_listing }
    }

    /// Refaz as subpastas da obra (versões e extras) a partir das listagens.
    fn refresh_work_dirs(&mut self) {
        let dirs = if self.season_name().is_some() {
            dirs(&self.season_listing)
        } else {
            self.title_dirs.iter().filter(|d| folder_year(&d.name).is_none()).cloned().collect()
        };
        self.work_split = split_work_dirs(dirs);
    }

    /// Nome da obra usado para rotular as versões.
    fn work_title(&self) -> String {
        folder_title(&self.lookup_name())
    }

    /// Versões da obra. Vídeos soltos ao lado de pastas de versão viram a
    /// versão sintética [`LOOSE_FILES_VERSION`], que aponta para a própria pasta.
    fn releases(&self) -> Vec<FileEntry> {
        let mut releases = self.work_split.0.clone();
        let loose = files_of(self.work_listing()).is_some_and(|f| !video_files(f).is_empty());
        if loose && !releases.is_empty() {
            releases.push(FileEntry {
                name: LOOSE_FILES_VERSION.to_owned(),
                is_directory: true,
                size: 0,
                modified: String::new(),
                extension: None,
            });
        }
        releases
    }

    /// Pasta extra aberta pelo caminho (`<obra>/Extras/...`).
    fn extra_name(&self) -> Option<String> {
        let rest = self.rest_after_season();
        let wanted = rest.first()?;
        self.work_split.1.iter().find(|e| &e.name == wanted).map(|e| e.name.clone())
    }

    /// Versão aberta: a do caminho; senão a lembrada ou a de maior resolução.
    fn release_name(&self, cx: &gpui_kit::App) -> Option<String> {
        let releases = self.releases();
        let rest = self.rest_after_season();
        if let Some(r) = rest.first().filter(|w| releases.iter().any(|r| &&r.name == w)) {
            return Some(r.clone());
        }
        let remembered = self.state.read(cx).core.prefs().version(&self.season_path());
        default_version(&releases, remembered.as_deref(), &self.work_title())
    }

    /// Pasta dos arquivos, ou `None` enquanto as listagens de cima não chegaram.
    fn files_path(&self, cx: &gpui_kit::App) -> Option<String> {
        self.title_listing.as_ref()?;
        if self.season_name().is_some() && self.season_listing.is_none() {
            return None;
        }
        let season_path = self.season_path();
        let rest = self.rest_after_season();
        if self.extra_name().is_some() {
            return Some(std::iter::once(season_path).chain(rest).collect::<Vec<_>>().join("/"));
        }
        let release = match self.release_name(cx) {
            None => return Some(season_path),
            Some(r) if r == LOOSE_FILES_VERSION => return Some(season_path),
            Some(r) => r,
        };
        let deeper: Vec<String> = if rest.first() == Some(&release) { rest[1..].to_vec() } else { Vec::new() };
        Some(std::iter::once(season_path).chain(std::iter::once(release)).chain(deeper).collect::<Vec<_>>().join("/"))
    }

    fn lookup_name(&self) -> String {
        self.season_name().unwrap_or_else(|| base_name(&self.title_path).to_owned())
    }

    fn summary(&self, cx: &gpui_kit::App) -> Option<Option<MediaSummary>> {
        self.state.read(cx).lookup(&self.lookup_name(), self.media_type)
    }

    fn media(&self, cx: &gpui_kit::App) -> Option<MediaSummary> {
        match &self.detail {
            Some(Some(d)) => Some(d.summary.clone()),
            _ => self.summary(cx).flatten(),
        }
    }

    fn full(&self) -> Option<&MediaDetail> {
        self.detail.as_ref().and_then(|d| d.as_ref())
    }

    /// Busca o que falta conforme as listagens e o AniList respondem.
    fn sync(&mut self, cx: &mut Context<Self>) {
        if self.title_listing.is_none() {
            return;
        }
        if self.shows_franchise() {
            if self.franchise.is_none() {
                let (path, works) = (self.title_path.clone(), self.title_dirs.clone());
                self.franchise = Some(cx.new(|cx| FranchiseView::new(path, works, cx)));
            }
            return;
        }
        if let Some(season) = self.season_name()
            && self.season_for.as_deref() != Some(season.as_str())
        {
            self.season_for = Some(season);
            self.season_listing = None;
            // Outra obra: nada do detalhe nem dos episódios da anterior fica.
            self.detail = None;
            self.detail_for = None;
            self.episodes_key = None;
            self.refresh_work_dirs();
            load_listing(&self.state, self.season_path(), cx, |v: &mut Self, r, cx| {
                v.season_listing = Some(r);
                v.refresh_work_dirs();
                v.sync(cx);
            });
        }
        if let Some(path) = self.files_path(cx)
            && self.files_for.as_deref() != Some(path.as_str())
        {
            self.files_for = Some(path.clone());
            self.files_listing = None;
            self.album = None;
            self.not_album = false;
            load_listing(&self.state, path, cx, |v: &mut Self, r, cx| {
                v.files_listing = Some(r);
                v.ensure_album(cx);
            });
        }
        let name = self.lookup_name();
        let media_type = self.media_type;
        let summary = self.summary(cx);
        if summary.is_none() {
            self.state.update(cx, |s, cx| s.want_lookup(&name, media_type, cx));
        }
        if let Some(Some(m)) = summary
            && self.detail_for != Some(m.id)
        {
            self.detail_for = Some(m.id);
            let core = self.state.read(cx).core.clone();
            let id = m.id;
            let task = core.clone().spawn(async move { core.anilist().media(id).await });
            cx.spawn(async move |this, cx| {
                let d = task.await.ok().and_then(Result::ok).and_then(|v| serde_json::from_value::<MediaDetail>(v).ok());
                let _ = this.update(cx, |v, cx| {
                    // A obra pode ter mudado enquanto o detalhe vinha.
                    if v.detail_for != Some(id) {
                        return;
                    }
                    v.detail = Some(d);
                    v.want_episodes(cx);
                    cx.notify();
                });
            })
            .detach();
        }
        self.want_episodes(cx);
    }

    /// Pede ao TMDB os episódios da obra (anime, fora filmes) quando o detalhe completo chega.
    fn want_episodes(&mut self, cx: &mut Context<Self>) {
        if self.media_type != MediaType::Anime {
            return;
        }
        // Só o detalhe da obra aberta: nunca misturar com o da anterior.
        let current = self.summary(cx).flatten().map(|m| m.id);
        let Some(d) = self.full().filter(|d| is_open_work(d.summary.id, self.detail_for, current)) else { return };
        let Some(q) = work_query(&d.summary, &self.lookup_name()) else { return };
        let key = work_key(&q);
        if self.episodes_key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.episodes_key = Some(key.clone());
        self.state.update(cx, |s, cx| s.want_episodes(key, q, cx));
    }

    /// Episódios do TMDB da obra aberta (`None` enquanto não chegam ou sem TMDB).
    fn tmdb_episodes(&self, cx: &gpui_kit::App) -> Option<Arc<WorkEpisodes>> {
        self.state.read(cx).episodes(self.episodes_key.as_deref()?).flatten()
    }

    /// Episódios para a aba: `None` enquanto o AniList ou o TMDB respondem;
    /// `Some(None)` sem dados do TMDB (mangá, filme, obra não achada, sem token).
    fn episodes_state(&self, cx: &gpui_kit::App) -> Option<Option<Arc<WorkEpisodes>>> {
        if self.media_type != MediaType::Anime {
            return Some(None);
        }
        match &self.episodes_key {
            Some(key) => self.state.read(cx).episodes(key),
            // Sem pedido ainda: espera o resumo e o detalhe da obra chegarem.
            None if self.detail.is_none() && !matches!(self.summary(cx), Some(None)) => None,
            None => Some(None),
        }
    }

    /// Arquivos que a tela mostra. Na raiz da obra (versão de arquivos soltos,
    /// ou obra só com extras) as subpastas ficam de fora: versões viram cartões
    /// e extras têm seção própria.
    fn shown_files(&self) -> Option<Vec<FileEntry>> {
        let list = files_of(&self.files_listing)?.clone();
        let at_root = self.files_for.as_deref() == Some(self.season_path().as_str());
        Some(if at_root { list.into_iter().filter(|f| !f.is_directory).collect() } else { list })
    }

    /// Pasta de músicas vira encarte (se não for álbum, ele avisa e volta à lista).
    fn ensure_album(&mut self, cx: &mut Context<Self>) {
        let Some(list) = files_of(&self.files_listing).cloned() else { return };
        if self.album.is_some() || self.not_album || !looks_like_album(&list) {
            return;
        }
        let (p, w) = (self.files_for.clone().unwrap_or_default(), Some(self.heading(cx)));
        let view = cx.new(|cx| AlbumView::new(p, list, w, cx));
        self._subs.push(cx.subscribe(&view, |v, _, e: &AlbumEvent, cx| {
            if matches!(e, AlbumEvent::NotAlbum) {
                v.not_album = true;
                cx.notify();
            }
        }));
        self.album = Some(view);
    }

    fn heading(&self, cx: &gpui_kit::App) -> String {
        self.media(cx).map_or_else(|| folder_title(&self.lookup_name()), |m| m.title())
    }

    fn play(&self, file: FileEntry, cx: &mut gpui_kit::App) {
        let (Some(dir), Some(files)) = (self.files_for.clone(), self.shown_files()) else { return };
        let t = PlayTarget {
            dir,
            file,
            files,
            mal_id: self.full().and_then(|d| d.summary.id_mal).and_then(|v| u32::try_from(v).ok()),
            is_anime: self.media_type == MediaType::Anime,
            anime_title: self.heading(cx),
        };
        self.state.update(cx, |s, cx| s.play(t, cx));
    }

    fn read(&self, file: FileEntry, cx: &mut gpui_kit::App) {
        let (Some(dir), Some(files)) = (self.files_for.clone(), self.shown_files()) else { return };
        let t = ReadTarget { dir, file, files, title_path: self.title_path.clone(), title: self.heading(cx) };
        self.state.update(cx, |s, cx| s.read(t, cx));
    }
}

impl Render for TitleView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(view) = self.franchise.clone().filter(|_| self.shows_franchise()) {
            return view.into_any_element();
        }
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, 72. * rem);
        let media = self.media(cx);
        let full = self.full().cloned();
        let heading = self.heading(cx);
        let release_name = self.release_name(cx);
        let files = self.shown_files();
        let files_path = self.files_for.clone().unwrap_or_default();
        let st = self.state.read(cx);
        let progress = st.player_data.progress.clone();
        let reading = progress_map(st);

        // --- Assistir / continuar ---
        let videos = files.as_ref().map(|f| video_files(f)).unwrap_or_default();
        let resume = {
            let mut last: Option<(usize, f64, bool, i64)> = None;
            for (i, f) in videos.iter().enumerate() {
                if let Some(p) = progress.get(&join_path(&files_path, &f.name))
                    && last.is_none_or(|l| p.updated_at >= l.3)
                {
                    last = Some((i, p.position, p.watched, p.updated_at));
                }
            }
            match last {
                Some((i, at, false, _)) => Some((videos[i].clone(), at)),
                Some((i, _, true, _)) => videos.get(i + 1).cloned().map(|f| (f, 0.)),
                None => videos.first().cloned().map(|f| (f, 0.)),
            }
        };
        let resume_label = |f: &FileEntry, at: f64| {
            let ep = episode_number(&f.name).map(|n| format!("ep. {n}")).unwrap_or_default();
            if at > 0. { format!("Continuar {ep} ({})", format_time(at)).replace("  ", " ") } else { format!("Assistir {ep}").trim().to_owned() }
        };

        // --- Ler / continuar ---
        let comics = files.as_ref().map(|f| comic_files(f)).unwrap_or_default();
        let resume_read = {
            let mut best: Option<(usize, usize, bool, i64, Option<String>)> = None;
            for (i, f) in comics.iter().enumerate() {
                if let Some(p) = reading.get(&join_path(&files_path, &f.name))
                    && best.as_ref().is_none_or(|b| p.updated_at >= b.3)
                {
                    best = Some((i, p.page, p.read, p.updated_at, p.unit_name.clone()));
                }
            }
            match best {
                None => comics.first().map(|f| (f.clone(), 0, false, None)),
                Some((i, page, false, _, unit)) => Some((comics[i].clone(), page, true, unit)),
                Some((i, _, true, _, _)) => comics.get(i + 1).map(|f| (f.clone(), 0, false, None)),
            }
        };
        let read_label = |f: &FileEntry, page: usize, resume: bool, unit: &Option<String>| {
            if resume && let Some(u) = unit {
                return format!("Continuar {}", label_text(&chapter_label(u)));
            }
            let l = label_text(&chapter_label(&f.name));
            if resume { format!("Continuar {l} (pág. {})", page + 1) } else { format!("Ler {l}") }
        };

        // --- Downloads ---
        let episode_paths = files.as_ref().map(|f| file_paths(&files_path, f)).unwrap_or_default();
        let remaining: Vec<String> = episode_paths
            .iter()
            .filter(|p| !matches!(st.download(p).map(|d| d.status), Some(DlStatus::Done | DlStatus::Running | DlStatus::Queued)))
            .cloned()
            .collect();

        // --- Abas ---
        let mut tabs = vec![(Section::Files, if self.media_type == MediaType::Manga { "Arquivos" } else { "Episódios" }, (!episode_paths.is_empty()).then_some(episode_paths.len()))];
        if let Some(d) = &full {
            if !d.characters.edges.is_empty() {
                tabs.push((Section::Cast, "Personagens", Some(d.characters.edges.len())));
            }
            if !d.staff.edges.is_empty() {
                let n = d.staff.edges.iter().map(|e| e.node.id).collect::<HashSet<_>>().len();
                tabs.push((Section::Staff, "Equipe", Some(n)));
            }
            if !d.relations.edges.is_empty() {
                tabs.push((Section::Related, "Relacionadas", Some(d.relations.edges.len())));
            }
            tabs.push((Section::Details, "Detalhes", None));
        }
        let active_ix = tabs.iter().position(|t| t.0 == self.tab).unwrap_or(0);
        let active_tab = tabs[active_ix].0;

        let tmdb = self.tmdb_episodes(cx);
        let episodes_state = self.episodes_state(cx);
        let rating = st.rating_source().work(media.as_ref(), tmdb.as_deref());
        let meta: Vec<String> = media
            .as_ref()
            .map(|m| {
                [
                    rating.map(|s| format!("★ {}", s.label())),
                    m.format.as_deref().map(|f| text::format(Some(f))),
                    m.year().map(|y| y.to_string()),
                    m.episodes.map(|e| format!("{e} {}", if e == 1 { "episódio" } else { "episódios" })),
                    m.chapters.map(|c| format!("{c} capítulos")),
                    full.as_ref().and_then(|d| d.studios.nodes.first().map(|s| s.name.clone())),
                ]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect()
            })
            .unwrap_or_default();

        let banner = media.as_ref().and_then(|m| m.banner_image.clone().or_else(|| m.cover().map(str::to_owned)));
        let has_banner = media.as_ref().is_some_and(|m| m.banner_image.is_some());
        let not_found = matches!(self.summary(cx), Some(None)) && self.title_listing.is_some();

        // --- Ações: uma principal (continuar de onde parou), o resto secundário ---
        let mut actions = h_flex().flex_wrap().gap_2();
        let mut has_primary = false;
        if let Some((f, at)) = resume.clone() {
            has_primary = true;
            actions = actions.child(
                Button::new("play-resume").primary().icon(IconName::Play).label(resume_label(&f, at)).on_click(cx.listener(move |v, _, _, cx| v.play(f.clone(), cx))),
            );
        }
        if let Some((f, page, res, unit)) = resume_read.clone() {
            let label = read_label(&f, page, res, &unit);
            let button = Button::new("read-resume").icon(IconName::BookOpen).label(label).on_click(cx.listener(move |v, _, _, cx| v.read(f.clone(), cx)));
            actions = actions.child(if has_primary { button } else { button.primary() });
        }
        let dl_label = if !remaining.is_empty() {
            format!("Baixar {} ({})", if remaining.len() == episode_paths.len() { "todos" } else { "restantes" }, remaining.len())
        } else if !episode_paths.is_empty() {
            "Tudo baixado ou na fila".into()
        } else {
            "Nada para baixar".into()
        };
        let st_entity = self.state.clone();
        let rem_paths = remaining.clone();
        actions = actions.child(
            Button::new("download-all").outline().icon(IconName::Download).label(dl_label).disabled(remaining.is_empty()).on_click(move |_, _, cx| {
                st_entity.update(cx, |s, cx| s.start_downloads(&rem_paths, cx));
            }),
        );
        // Fora de franquia abre o título inteiro: irmãs sem data (Filmes, Extras) ficam ao alcance.
        let raw = raw_path(&if self.is_franchise() { self.season_path() } else { self.title_path.clone() });
        let st_entity = self.state.clone();
        actions = actions.child(Button::new("raw-folder").ghost().icon(IconName::Folder).label("Ver pasta").on_click(move |_, _, cx| {
            st_entity.update(cx, |s, cx| s.go(raw.clone(), cx));
        }));
        // Destinos externos (abrem no navegador): Link, não botão.
        let mut links = h_flex().gap_4().text_sm();
        if let Some(t) = full.as_ref().and_then(|d| d.trailer.clone()).filter(|t| t.site.as_deref() == Some("youtube")) {
            links = links.child(Link::new("trailer").href(format!("https://www.youtube.com/watch?v={}", t.id)).child("Trailer no YouTube"));
        }
        if let Some(url) = full.as_ref().and_then(|d| d.site_url.clone()) {
            let mal_only = full.as_ref().is_some_and(|d| d.summary.is_mal_only());
            links = links.child(Link::new("site").href(url).child(if mal_only { "Ver no MyAnimeList" } else { "Página no AniList" }));
        }

        // --- Sinopse ---
        let synopsis = full.as_ref().map(|d| text::clean_description(d.description.as_deref())).filter(|(p, _)| !p.is_empty()).map(|(paras, source)| {
            let long = paras.join(" ").len() > 300;
            let expanded = self.expanded;
            v_flex()
                .max_w(px(42. * rem))
                .gap_2()
                .child(
                    v_flex()
                        .gap_3()
                        .text_color(theme.foreground.opacity(0.85))
                        .line_height(relative(1.6))
                        .when(long && !expanded, |d| d.line_clamp(3))
                        .children(if long && !expanded { vec![paras.join(" ")] } else { paras.clone() }),
                )
                .when(long || source.is_some(), |d| {
                    d.child(
                        h_flex()
                            .gap_3()
                            .when(long, |d| {
                                d.child(Button::new("more").text().xsmall().label(if expanded { "Mostrar menos" } else { "Mostrar mais" }).on_click(cx.listener(|v, _, _, cx| {
                                    v.expanded = !v.expanded;
                                    cx.notify();
                                })))
                            })
                            .when_some(source.filter(|_| expanded || !long), |d, s| d.child(div().text_xs().text_color(theme.muted_foreground).child(format!("Fonte: {s}")))),
                    )
                })
        });

        // --- Aba de arquivos ---
        let files_tab = {
            let seasons = self.seasons();
            let releases = self.releases();
            let season_name = self.season_name();
            let work_path = self.season_path();
            let work_title = self.work_title();
            let selected = release_name.clone().filter(|_| self.extra_name().is_none());
            let single_tags = if releases.len() == 1 { release_tags(&releases[0].name) } else { Vec::new() };
            let mut options = v_flex().gap_3();
            // Franquias trocam de obra pela página da franquia (link de volta acima do título).
            if seasons.len() > 1 && !self.is_franchise() {
                options = options.child(option_group("Temporada", cx).child(h_flex().flex_wrap().gap_2().children(seasons.iter().map(|s| {
                    let target = join_path(&self.title_path, &s.name);
                    let st = self.state.clone();
                    let year = folder_year(&s.name).map(|y| format!("{y} · ")).unwrap_or_default();
                    option_button(ElementId::Name(format!("season:{}", s.name).into()), format!("{year}{}", folder_title(&s.name)), Some(&s.name) == season_name.as_ref())
                        .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx)))
                }))));
            }
            if releases.len() > 1 {
                // Só a versão aberta foi listada: as outras ficam sem contagem.
                let count = files.as_ref().filter(|_| self.album.is_none()).map(|f| video_files(f).len()).filter(|&n| n > 0);
                options = options.child(option_group("Versão", cx).child(h_flex().flex_wrap().gap_2().children(releases.iter().map(|r| {
                    let on = Some(&r.name) == selected.as_ref();
                    let target = if r.name == LOOSE_FILES_VERSION { work_path.clone() } else { join_path(&work_path, &r.name) };
                    let (st, work, folder) = (self.state.clone(), work_path.clone(), r.name.clone());
                    version_card(ElementId::Name(format!("release:{}", r.name).into()), &label_of(&r.name, &work_title), count.filter(|_| on), on, cx)
                        .on_click(move |_, _, cx| {
                            st.update(cx, |s, cx| {
                                s.core.prefs().set_version(&work, &folder);
                                s.go(target.clone(), cx);
                            })
                        })
                }))));
            }
            if !single_tags.is_empty() {
                options = options.child(option_group("Versão", cx).child(div().pt_1().text_sm().text_color(theme.muted_foreground).child(single_tags.join(" "))));
            }
            let extras = &self.work_split.1;
            let extras_section = (!extras.is_empty()).then(|| {
                v_flex().gap_2().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Extras")).child(h_flex().flex_wrap().gap_2().children(extras.iter().map(|e| {
                    let target = join_path(&work_path, &e.name);
                    let st = self.state.clone();
                    Button::new(ElementId::Name(format!("extra:{}", e.name).into()))
                        .outline()
                        .small()
                        .icon(IconName::Folder)
                        .label(folder_title(&e.name))
                        .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx)))
                })))
            });
            let body = match (&self.files_listing, files.clone()) {
                (Some(Err(e)), _) => empty_state(IconName::WifiOff, "Não foi possível abrir os arquivos", Some(e.clone().into()), cx).into_any_element(),
                (_, None) => Skeleton::new().h_72().rounded(theme.radius_lg).into_any_element(),
                (_, Some(_)) if self.album.is_some() && !self.not_album => self.album.clone().unwrap().into_any_element(),
                (_, Some(list)) => {
                    let this = cx.entity().downgrade();
                    let on_play: FileAction = Rc::new({
                        let this = this.clone();
                        move |f, _, cx| {
                            let _ = this.update(cx, |v, cx| v.play(f.clone(), cx));
                        }
                    });
                    let on_read: FileAction = Rc::new(move |f, _, cx| {
                        let _ = this.update(cx, |v, cx| v.read(f.clone(), cx));
                    });
                    if self.media_type == MediaType::Anime {
                        let layout = self.state.read(cx).episode_layout();
                        let cols = columns(width, 15.5 * rem, rem);
                        let eps = episodes_state.as_ref().map(|e| e.as_deref());
                        episode_list(&files_path, &list, eps, layout, cols, on_play, Some(on_read), cx).into_any_element()
                    } else {
                        file_list(&files_path, &list, Some(on_play), Some(on_read), cx).into_any_element()
                    }
                }
            };
            v_flex().gap_5().child(options).child(body).children(extras_section)
        };

        let panel: AnyElement = match (active_tab, &full) {
            (Section::Cast, Some(d)) => cast(d, width, rem, cx).into_any_element(),
            (Section::Staff, Some(d)) => staff(d, width, rem, cx).into_any_element(),
            (Section::Related, Some(d)) => related(d, width, rem, cx).into_any_element(),
            (Section::Details, Some(d)) => facts(d, cx).into_any_element(),
            _ => files_tab.into_any_element(),
        };

        let native = media.as_ref().and_then(|m| m.title.native.clone()).filter(|n| *n != heading);
        let english = media.as_ref().and_then(|m| m.title.english.clone()).filter(|n| *n != heading);
        let genres = media.as_ref().map(|m| m.genres.iter().map(|g| text::genre(g)).collect::<Vec<_>>().join(", ")).filter(|g| !g.is_empty());
        let tab_ids: Vec<Section> = tabs.iter().map(|t| t.0).collect();
        let back = self.is_franchise().then(|| {
            let (st, target) = (self.state.clone(), self.title_path.clone());
            h_flex().child(
                Button::new("franchise-back")
                    .ghost()
                    .small()
                    .icon(IconName::ChevronLeft)
                    .label(folder_title(base_name(&self.title_path)))
                    .on_click(move |_, _, cx| st.update(cx, |s, cx| s.go(target.clone(), cx))),
            )
        });

        div()
            .relative()
            .min_h_full()
            // Banner esmaecido no topo: imagem é mídia, o véu vem do fundo do tema.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h_96()
                    .overflow_hidden()
                    .children(banner.map(|b| div().absolute().inset_0().opacity(if has_banner { 0.45 } else { 0.25 }).child(remote_image(Some(&b), cx))))
                    .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                        180.,
                        gpui_kit::linear_color_stop(theme.background.opacity(0.3), 0.),
                        gpui_kit::linear_color_stop(theme.background, 1.),
                    )))
                    // Véu do lado do texto: sinopse e ações legíveis sobre qualquer banner.
                    .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                        90.,
                        gpui_kit::linear_color_stop(theme.background.opacity(0.85), 0.25),
                        gpui_kit::linear_color_stop(theme.background.opacity(0.), 0.9),
                    ))),
            )
            .child(
                v_flex()
                    .relative()
                    .w(px(width))
                    .mx_auto()
                    .pt_6()
                    .pb_16()
                    .gap_8()
                    .child(breadcrumbs(&self.title_path, cx))
                    .child(
                        h_flex()
                            .mt_12()
                            .items_start()
                            .gap_8()
                            .child(div().flex_none().shadow_lg().rounded(theme.radius_lg).child(cover(media.as_ref(), &self.lookup_name(), Some(px(11.5 * rem)), cx)))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_4()
                                    .children(back)
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .children(native.map(|n| div().text_sm().text_color(theme.muted_foreground).child(n)))
                                            .child(div().text_3xl().line_height(relative(1.1)).font_weight(FontWeight::BOLD).child(heading.clone()))
                                            .children(english.map(|n| div().text_color(theme.muted_foreground).child(n))),
                                    )
                                    .when(!meta.is_empty() || genres.is_some(), |d| {
                                        d.child(
                                            v_flex()
                                                .gap_1()
                                                .when(!meta.is_empty(), |d| d.child(div().text_sm().text_color(theme.muted_foreground).child(meta.join(" · "))))
                                                .children(genres.map(|g| div().text_sm().child(g))),
                                        )
                                    })
                                    .children(synopsis)
                                    .when(not_found, |d| {
                                        d.child(div().max_w(px(38. * rem)).text_sm().text_color(theme.muted_foreground).child("Não achamos este título no AniList. Os arquivos continuam disponíveis abaixo."))
                                    })
                                    .child(actions)
                                    .child(links),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_6()
                            .child(
                                TabBar::new("title-tabs")
                                    .underline()
                                    .selected_index(active_ix)
                                    .on_click(cx.listener(move |v, ix: &usize, _, cx| {
                                        if let Some(t) = tab_ids.get(*ix) {
                                            v.tab = *t;
                                            cx.notify();
                                        }
                                    }))
                                    .children(tabs.into_iter().map(|(_, label, count)| match count {
                                        Some(n) => Tab::new().cursor_pointer().label(format!("{label} {n}")),
                                        None => Tab::new().cursor_pointer().label(label),
                                    })),
                            )
                            .child(panel),
                    ),
            )
            .into_any_element()
    }
}

fn is_placeholder(url: Option<&str>) -> bool {
    url.is_none_or(|u| u.ends_with("/default.jpg") || u.ends_with("/default.png"))
}

/// Foto de pessoa/personagem; iniciais quando não há imagem.
fn portrait(url: Option<&str>, name: Option<&str>, cx: &mut gpui_kit::App) -> AnyElement {
    if is_placeholder(url) {
        Avatar::new().name(name.unwrap_or("?").to_owned()).into_any_element()
    } else {
        div().size_12().flex_none().rounded_full().overflow_hidden().bg(cx.theme().muted).child(rounded_image(url, Some(px(999.)), cx)).into_any_element()
    }
}

fn cast(d: &MediaDetail, width: f32, rem: f32, cx: &mut gpui_kit::App) -> gpui_kit::Div {
    let theme = cx.theme().clone();
    let cols = columns(width, 20. * rem, 2. * rem);
    div().grid().grid_cols(cols as u16).gap_x_8().children(d.characters.edges.iter().map(|e| {
        let va = e.voice_actors.first();
        h_flex()
            .gap_3()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .child(portrait(e.node.image.as_ref().and_then(|i| i.large.as_deref()), e.node.name.full.as_deref(), cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(truncate(e.node.name.full.clone().unwrap_or_default()).text_sm().font_weight(FontWeight::SEMIBOLD))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(text::character_role(Some(&e.role)))),
            )
            .children(va.map(|va| {
                h_flex()
                    .gap_3()
                    .flex_1()
                    .min_w_0()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .items_end()
                            .child(truncate(va.name.full.clone().unwrap_or_default()).text_sm())
                            .child(div().text_xs().text_color(theme.muted_foreground).child(va.name.native.clone().unwrap_or_default())),
                    )
                    .child(portrait(va.image.as_ref().and_then(|i| i.large.as_deref()), va.name.full.as_deref(), cx))
            }))
    }))
}

fn staff(d: &MediaDetail, width: f32, rem: f32, cx: &mut gpui_kit::App) -> gpui_kit::Div {
    let theme = cx.theme().clone();
    // Agrupa papéis da mesma pessoa: "Direção, Criação original".
    let mut people: Vec<(&higanbana_core::anilist::types::Person, Vec<String>)> = Vec::new();
    for e in &d.staff.edges {
        match people.iter_mut().find(|(p, _)| p.id == e.node.id) {
            Some((_, roles)) => roles.push(text::staff_role(&e.role)),
            None => people.push((&e.node, vec![text::staff_role(&e.role)])),
        }
    }
    let cols = columns(width, 16. * rem, 2. * rem);
    div().grid().grid_cols(cols as u16).gap_x_8().children(people.into_iter().map(|(p, roles)| {
        h_flex()
            .gap_3()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .child(portrait(p.image.as_ref().and_then(|i| i.large.as_deref()), p.name.full.as_deref(), cx))
            .child(
                v_flex()
                    .min_w_0()
                    .child(truncate(p.name.full.clone().unwrap_or_default()).text_sm().font_weight(FontWeight::SEMIBOLD))
                    .child(div().text_xs().text_color(theme.muted_foreground).line_clamp(2).child(roles.join(", "))),
            )
    }))
}

/// Obras relacionadas: abrem no AniList (destino externo).
fn related(d: &MediaDetail, width: f32, rem: f32, cx: &mut gpui_kit::App) -> gpui_kit::Div {
    let theme = cx.theme().clone();
    let gap = 1.25 * rem;
    let cols = columns(width, 8.25 * rem, gap);
    let card_w = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());
    v_flex().gap_5().child(div().text_sm().text_color(theme.muted_foreground).child("Abrem no AniList, no navegador.")).child(
        div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children(d.relations.edges.iter().map(|e| {
            let n = &e.node;
            let kind = n.media_type.map_or("anime", |t| if t == MediaType::Manga { "manga" } else { "anime" });
            let url = format!("https://anilist.co/{kind}/{}", n.id);
            let bg = n.cover_image.as_ref().and_then(|c| hex_color(c.color.as_deref())).unwrap_or(theme.muted);
            v_flex()
                .id(ElementId::Name(format!("rel:{}", n.id).into()))
                .w(card_w)
                .gap_1()
                .cursor_pointer()
                .child(
                    div()
                        .mb_1()
                        .w(card_w)
                        .h(card_w * 1.5)
                        .rounded(theme.radius_lg)
                        .overflow_hidden()
                        .bg(bg)
                        .relative()
                        .child(rounded_image(n.cover_image.as_ref().and_then(|c| c.large.as_deref()), Some(theme.radius_lg), cx))
                        .child(hover_layer("ring").border(theme.ring).radius(theme.radius_lg)),
                )
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.primary_ink()).child(text::relation(Some(&e.relation_type))))
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).line_clamp(2).child(n.title.user_preferred.clone().unwrap_or_default()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{} {}", text::format(n.format.as_deref()), n.start_date.and_then(|d| d.year).map(|y| y.to_string()).unwrap_or_default())),
                )
                .on_click(move |_, _, cx| cx.open_url(&url))
        })),
    )
}

type FactGroup<'a> = (&'a str, Vec<(&'a str, Option<String>)>);

fn facts(d: &MediaDetail, cx: &gpui_kit::App) -> gpui_kit::Div {
    let site = if d.summary.is_mal_only() { "MyAnimeList" } else { "AniList" };
    let theme = cx.theme().clone();
    let m = &d.summary;
    let groups: Vec<FactGroup> = vec![
        (
            "Obra",
            vec![
                ("Formato", m.format.as_deref().map(|f| text::format(Some(f)))),
                ("Origem", d.source.as_deref().map(|s| text::source(Some(s)))),
                ("Estúdio", Some(d.studios.nodes.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty())),
                ("Episódios", m.episodes.map(|e| e.to_string())),
                ("Duração", d.duration.map(|x| format!("{x} min por episódio"))),
                ("Capítulos", m.chapters.map(|c| c.to_string())),
                ("Volumes", d.volumes.map(|v| v.to_string())),
            ],
        ),
        (
            "Exibição",
            vec![
                ("Situação", m.status.as_deref().map(|s| text::status(Some(s)))),
                ("Temporada", d.summary.season.as_deref().zip(m.season_year).map(|(s, y)| format!("{} de {y}", text::season(Some(s))))),
                ("Estreia", text::date(m.start_date)),
                ("Fim", text::date(d.end_date)),
            ],
        ),
        (
            site,
            vec![
                ("Nota média", m.average_score.map(|s| format!("{s}%"))),
                ("Popularidade", m.popularity.map(|p| format!("{} pessoas", text::number(p)))),
                ("Outros nomes", Some(m.synonyms.iter().take(4).cloned().collect::<Vec<_>>().join("; ")).filter(|s| !s.is_empty())),
            ],
        ),
    ];
    let tags: Vec<&str> = d.tags.iter().filter(|t| !t.spoiler && t.rank >= 60).take(12).map(|t| t.name.as_str()).collect();
    v_flex()
        .gap_8()
        .child(div().grid().grid_cols(3).gap_8().children(groups.into_iter().filter_map(|(g, rows)| {
            let rows: Vec<(&str, String)> = rows.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect();
            (!rows.is_empty()).then(|| {
                v_flex().child(div().mb_2().text_sm().font_weight(FontWeight::SEMIBOLD).child(g)).children(rows.into_iter().map(|(k, v)| {
                    h_flex()
                        .items_start()
                        .justify_between()
                        .gap_6()
                        .py_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .text_sm()
                        .child(div().flex_none().text_color(theme.muted_foreground).child(k))
                        .child(div().text_right().child(v))
                }))
            })
        })))
        .when(!tags.is_empty(), |d| {
            d.child(
                v_flex().gap_2().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Temas")).child(
                    h_flex().flex_wrap().gap_1p5().children(tags.into_iter().map(|t| Tag::secondary().small().child(t.to_owned()))),
                ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> FileEntry {
        FileEntry { name: name.to_owned(), is_directory: true, size: 0, modified: String::new(), extension: None }
    }

    fn smile() -> Vec<FileEntry> {
        vec![
            dir("[Aenianos] Smile Precure [1280x720][hi10p]"),
            dir("[DragsterPS] Glitter Force S01 e S02 [1080p] [Multi-Audio] [Multi-Subs]"),
            dir("Extras"),
        ]
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
        // Trocou de obra: o resumo novo ainda não chegou ou já é outro.
        assert!(!is_open_work(7, Some(7), None));
        assert!(!is_open_work(7, Some(7), Some(8)));
        // O detalhe pedido agora é outro (ou foi zerado na troca).
        assert!(!is_open_work(7, Some(8), Some(8)));
        assert!(!is_open_work(7, None, Some(7)));
    }
}
