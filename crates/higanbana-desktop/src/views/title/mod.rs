//! Página de um título: banner esmaecido, capa, nomes, notas, sinopse, ações e
//! abas (Episódios/Arquivos, Personagens, Equipe, Relacionadas, Detalhes).
//! Franquia sem obra escolhida mostra a tela da franquia.

pub mod data;
mod files_tab;
mod header;
mod people;
mod related;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use higanbana_core::anilist::{types::{MediaDetail, MediaSummary}, MediaType};
use higanbana_core::nuvem::FileEntry;
use higanbana_core::tmdb::{matching::work_query, work_key, WorkEpisodes};
use higanbana_gpui::components::{Component, TabBar};
use higanbana_gpui::prelude::*;

use data::{is_open_work, of, shown_files, Model};

use crate::root::Higanbana;
use crate::ui::cover::content_width;
use crate::ui::paths::breadcrumbs;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Files,
    Cast,
    Staff,
    Related,
    Details,
}

/// Estado de interface do título (some ao sair dele).
#[derive(Default)]
pub struct TitleUi {
    pub tab: Section,
    pub expanded: bool,
    /// Aba da franquia escolhida; `None` = a primeira que existir.
    pub franchise_tab: Option<higanbana_core::franchise::FranchiseTab>,
}

/// O que as partes da tela precisam, já resolvido neste render.
pub struct Work {
    pub model: Model,
    pub summary: Option<Option<Arc<MediaSummary>>>,
    pub detail: Option<Arc<MediaDetail>>,
    pub media: Option<MediaSummary>,
    pub heading: String,
    pub files_path: Option<String>,
    pub files: Option<Result<Vec<FileEntry>, String>>,
    pub episodes: Option<Option<Arc<WorkEpisodes>>>,
    pub release: Option<String>,
}

/// Monta a obra aberta e pede o que falta (listagens, AniList, TMDB).
fn work(v: &mut Higanbana, title_path: &str, rest: &[String], media_type: MediaType) -> Work {
    let s = &mut v.state;
    let model = Model::load(s, title_path, rest, media_type);
    let remembered = s.core.prefs().version(&model.season_path());
    let release = model.release_name(remembered.as_deref());
    let files_path = model.files_path(remembered.as_deref());
    let at_root = files_path.as_deref() == Some(model.season_path().as_str());
    let files = files_path.as_ref().and_then(|p| of(s.listing(p))).map(|r| r.map(|l| shown_files(&l, at_root)));
    let name = model.lookup_name();
    let summary = s.lookup(&name, media_type);
    if summary.is_none() && model.title.is_some() && !model.shows_franchise() {
        s.want_lookup(&name, media_type);
    }
    let detail = summary.clone().flatten().and_then(|m| s.media_detail(m.id)).flatten();
    // Episódios do TMDB só com o detalhe da obra aberta (anime).
    let current = summary.clone().flatten().map(|m| m.id);
    let mut episodes = if media_type == MediaType::Anime { None } else { Some(None) };
    if media_type == MediaType::Anime {
        match detail.as_ref().filter(|d| is_open_work(d.summary.id, current, current)).and_then(|d| work_query(d, &name)) {
            Some(q) => {
                let key = work_key(&q);
                s.want_episodes(key.clone(), q);
                episodes = s.episodes(&key);
            }
            None if detail.is_some() || matches!(summary, Some(None)) => episodes = Some(None),
            None => {}
        }
    }
    let media = detail.as_ref().map(|d| d.summary.clone()).or_else(|| summary.clone().flatten().map(|m| (*m).clone()));
    let heading = media.as_ref().map_or_else(|| model.work_title(), |m| m.title());
    Work { model, summary, detail, media, heading, files_path, files, episodes, release }
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, title_path: &str, rest: &[String], media_type: MediaType) -> Element<Higanbana> {
    let w = work(v, title_path, rest, media_type);
    if w.model.shows_franchise() {
        return crate::views::franchise::page(v, cx, &w.model);
    }
    let t = cx.theme().clone();
    let width = content_width(v, cx, 1152.0);
    let tabs = tabs(&w, media_type);
    let ui_tab = v.pages.title().tab;
    let active = tabs.iter().position(|x| x.0 == ui_tab).unwrap_or(0);
    let ids: Vec<Section> = tabs.iter().map(|x| x.0).collect();
    let mut bar = TabBar::new("titulo-abas", active, move |v: &mut Higanbana, i, _| {
        if let Some(s) = ids.get(i) {
            v.pages.title().tab = *s;
        }
    })
    .underline();
    for (_, label, count) in &tabs {
        bar = bar.tab(match count {
            Some(n) => format!("{label} {n}"),
            None => (*label).to_owned(),
        });
    }
    let bar = bar.render(cx);
    let panel = match (tabs[active].0, &w.detail) {
        (Section::Cast, Some(d)) => people::cast(v, cx, d, width),
        (Section::Staff, Some(d)) => people::staff(v, cx, d, width),
        (Section::Related, Some(d)) => related::related(v, cx, d, width),
        (Section::Details, Some(d)) => related::facts(cx, d),
        _ => files_tab::files_tab(v, cx, &w, width),
    };
    let body = div()
        .relative()
        .w(width)
        .flex_col()
        .pt(24.0)
        .pb(64.0)
        .gap(32.0)
        .child(breadcrumbs(cx, title_path, false))
        .child(header::head(v, cx, &w))
        .child(div().flex_col().gap(24.0).child(bar).child(panel));
    div().relative().w_full().min_h(pct(100.0)).flex_col().items_center().bg(t.background).child(header::backdrop(v, cx, &w)).child(body)
}

/// Abas que existem para esta obra: (seção, rótulo, contagem).
fn tabs(w: &Work, media: MediaType) -> Vec<(Section, &'static str, Option<usize>)> {
    let n = w.files.as_ref().and_then(|r| r.as_ref().ok()).map_or(0, |f| f.iter().filter(|f| !f.is_directory).count());
    let mut tabs = vec![(Section::Files, if media == MediaType::Manga { "Arquivos" } else { "Episódios" }, (n > 0).then_some(n))];
    if let Some(d) = &w.detail {
        if !d.characters.edges.is_empty() {
            tabs.push((Section::Cast, "Personagens", Some(d.characters.edges.len())));
        }
        if !d.staff.edges.is_empty() {
            let n = d.staff.edges.iter().map(|e| e.node.id).collect::<std::collections::HashSet<_>>().len();
            tabs.push((Section::Staff, "Equipe", Some(n)));
        }
        if !d.relations.edges.is_empty() {
            tabs.push((Section::Related, "Relacionadas", Some(d.relations.edges.len())));
        }
        tabs.push((Section::Details, "Detalhes", None));
    }
    tabs
}
