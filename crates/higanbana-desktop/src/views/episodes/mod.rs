//! Aba Episódios de um anime: cada vídeo vira um card (grade) ou uma linha
//! (lista) com miniatura, título e nota do TMDB quando o arquivo casa com um
//! episódio. Subpastas e outros arquivos ficam na lista compacta, abaixo.

mod cards;
mod labels;
mod shapes;
mod still;
#[cfg(test)]
mod tests;

use higanbana_core::library::join_path;
use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::logic::video_files;
use higanbana_core::settings::EpisodeLayout;
use higanbana_core::tmdb::{still_url, EpisodeView, WorkEpisodes};
use higanbana_gpui::components::{Button, Component, ComponentSize, Variant};
use higanbana_gpui::prelude::*;

use labels::{air_date, card_text, disambiguators, list_columns};

use crate::chrome::{format_bytes, tip};
use crate::root::Higanbana;
use crate::ui::files::{file_list, sorted, FileAction, FileRows};

/// Tamanho do still pedido ao TMDB e o minúsculo do fundo borrado da lista.
const STILL_SIZE: &str = "w300";
const BLEED_SIZE: &str = "w92";

/// O que um card precisa saber de um vídeo, já lido do estado.
pub struct Item {
    pub file: FileEntry,
    pub path: String,
    pub label: String,
    pub title: String,
    pub note: Option<String>,
    pub details: String,
    pub still: Option<String>,
    pub bleed: Option<String>,
    pub score: Option<String>,
    pub date: Option<String>,
    pub overview: String,
    pub minutes: Option<u32>,
    pub seen: Option<f32>,
    pub watched: bool,
}

fn items(v: &Higanbana, dir: &str, videos: Vec<FileEntry>, tmdb: Option<&WorkEpisodes>) -> Vec<Item> {
    let names: Vec<&str> = videos.iter().map(|f| f.name.as_str()).collect();
    let links: Vec<Option<&EpisodeView>> = tmdb.map_or_else(|| vec![None; names.len()], |w| w.links(&names));
    let single = tmdb.is_some_and(|w| w.single);
    let texts: Vec<(String, String)> = videos.iter().zip(&links).map(|(f, ep)| card_text(f, *ep, single)).collect();
    let mut notes = disambiguators(&texts.iter().zip(&videos).map(|((l, _), f)| (l.clone(), f.name.clone())).collect::<Vec<_>>());
    // Obra de um episódio só com vários arquivos: são versões, o nome separa.
    if single && videos.len() > 1 {
        notes = videos.iter().map(|f| Some(f.name.clone())).collect();
    }
    let ratings = v.state.rating_source();
    videos
        .into_iter()
        .zip(links.iter().zip(texts.into_iter().zip(notes)))
        .map(|(f, (&ep, ((label, title), note)))| {
            let path = join_path(dir, &f.name);
            let ext = f.extension.clone().unwrap_or_default();
            let p = v.state.player_data.progress.get(&path);
            let minutes = ep.and_then(|e| e.runtime).filter(|&m| m > 0).or_else(|| p.filter(|p| p.duration > 0.).map(|p| (p.duration / 60.).round() as u32));
            Item {
                label,
                title,
                note,
                details: [ext.trim_start_matches('.').to_uppercase(), format_bytes(f.size)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "),
                still: ep.and_then(|e| e.still_path.as_deref()).map(|p| still_url(p, STILL_SIZE)),
                bleed: ep.and_then(|e| e.still_path.as_deref()).map(|p| still_url(p, BLEED_SIZE)),
                score: ep.and_then(|e| ratings.episode(e)).map(|s| s.label()),
                date: ep.and_then(|e| air_date(e.air_date.as_deref())),
                overview: ep.map(|e| e.overview.clone()).unwrap_or_default(),
                minutes,
                seen: p.map(|p| if p.watched { 1. } else { (p.position / p.duration.max(1.)).clamp(0., 1.) as f32 }),
                watched: p.is_some_and(|p| p.watched),
                file: f,
                path,
            }
        })
        .collect()
}

/// Lista de episódios. `eps`: `None` enquanto o TMDB responde; `Some(None)` sem TMDB.
pub fn episode_list(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, dir: &str, files: &[FileEntry], eps: Option<Option<&WorkEpisodes>>, width: f32, on_play: FileAction, on_read: Option<FileAction>) -> Element<Higanbana> {
    let videos = video_files(files);
    if videos.is_empty() {
        return file_list(v, cx, &FileRows::new(dir, sorted(files), false, Some(on_play), on_read));
    }
    let others: Vec<FileEntry> = files.iter().filter(|f| !videos.iter().any(|x| x.name == f.name)).cloned().collect();
    let loading = eps.is_none();
    let items = items(v, dir, videos, eps.flatten());
    let layout = v.state.episode_layout();
    let (icon_name, hint, next) = match layout {
        EpisodeLayout::Grid => ("layout-list", "Mostrar em lista", EpisodeLayout::List),
        EpisodeLayout::List => ("layout-grid", "Mostrar em grade", EpisodeLayout::Grid),
    };
    let toggle = Button::new("episodios-layout", "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon(icon_name).size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
        v.state.set_episode_layout(next);
        cx.notify();
    });
    let toggle = toggle.render(cx);
    let toggle = tip(v, cx, "dica-episodios-layout", toggle, hint);
    let cols = crate::ui::cover::columns(width, 248.0, 16.0);
    let body = match layout {
        EpisodeLayout::Grid => {
            let w = (width - 16.0 * (cols as f32 - 1.0)) / cols as f32;
            let mut g = div().flex_wrap().gap_x(16.0).gap_y(24.0).items_start();
            for (i, it) in items.iter().enumerate() {
                g = g.child(cards::grid_card(v, cx, it, i, loading, &on_play, w));
            }
            g
        }
        EpisodeLayout::List => {
            let lc = list_columns(cols);
            let w = (width - 12.0 * (lc as f32 - 1.0)) / lc as f32;
            let mut g = div().flex_wrap().gap(12.0).items_start();
            for (i, it) in items.iter().enumerate() {
                g = g.child(cards::list_card(v, cx, it, i, loading, &on_play, w));
            }
            g
        }
    };
    let mut col = div().flex_col().gap(12.0).child(div().justify_end().child(toggle)).child(body);
    if !others.is_empty() {
        col = col.child(div().pt(12.0).child(file_list(v, cx, &FileRows::new(dir, sorted(&others), false, None, on_read))));
    }
    col
}
