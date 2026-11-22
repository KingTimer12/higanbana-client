//! Topo do título: banner esmaecido com véus, capa, nomes, notas, gêneros,
//! sinopse (3 linhas, "Mostrar mais"), ações e links externos.

use higanbana_core::anilist::{text as anitext, MediaType};
use higanbana_core::library::{base_name, folder_title, join_path, raw_path};
use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::logic::{episode_number, format_time, video_files};
use higanbana_core::reader::logic::{chapter_label, comic_files, label_text};
use higanbana_gpui::components::{Button, Component, ComponentSize, Link, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};

use super::Work;
use crate::root::Higanbana;
use crate::state::details::open_url;
use crate::state::DlStatus;
use crate::ui::cover::cover;
use crate::ui::files::FileAction;
use crate::ui::image::remote_image;
use crate::ui::SHADOW_LG;
use crate::video::PlayTarget;

/// Altura do banner no topo (`h_96`).
const BANNER: f32 = 384.0;

pub fn backdrop(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: &Work) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let banner = w.media.as_ref().and_then(|m| m.banner_image.clone().or_else(|| m.cover().map(str::to_owned)));
    let has_banner = w.media.as_ref().is_some_and(|m| m.banner_image.is_some());
    let mut d = div().absolute().top(0.0).left(0.0).right(0.0).h(BANNER).overflow_hidden();
    if let Some(b) = banner {
        d = d.child(div().absolute().top(0.0).left(0.0).size_full().opacity(if has_banner { 0.45 } else { 0.25 }).child(remote_image(v, cx, Some(&b), 0.0)));
    }
    let down = LinearGradient::new(GradientDir::Down, vec![(0.0, t.background.fade(0.3)), (1.0, t.background)]);
    let side = LinearGradient::new(GradientDir::Right, vec![(0.25, t.background.fade(0.85)), (0.9, t.background.fade(0.0))]);
    d.child(div().absolute().top(0.0).left(0.0).size_full().bg_gradient(down)).child(div().absolute().top(0.0).left(0.0).size_full().bg_gradient(side))
}

/// Toca `file` da pasta aberta.
pub fn play_action(w: &Work) -> Option<FileAction> {
    let (dir, files) = (w.files_path.clone()?, w.files.as_ref()?.as_ref().ok()?.clone());
    let mal_id = w.detail.as_ref().and_then(|d| d.summary.id_mal).and_then(|v| u32::try_from(v).ok());
    let (is_anime, title) = (w.model.media == MediaType::Anime, w.heading.clone());
    Some(std::rc::Rc::new(move |v: &mut Higanbana, f: &FileEntry, cx: &mut EventCx| {
        v.state.play(PlayTarget { dir: dir.clone(), file: f.clone(), files: files.clone(), mal_id, is_anime, anime_title: title.clone() });
        cx.notify();
    }))
}

/// Ler: abre o leitor no arquivo; preferências guardadas pelo caminho da obra.
pub fn read_action(w: &Work) -> Option<FileAction> {
    let dir = w.files_path.clone()?;
    let files: std::sync::Arc<[FileEntry]> = w.files.as_ref()?.as_ref().ok()?.iter().filter(|f| higanbana_core::reader::logic::is_comic(f)).cloned().collect();
    let (title_path, title) = (w.model.title_path.clone(), w.heading.clone());
    Some(std::rc::Rc::new(move |v: &mut Higanbana, f: &FileEntry, cx: &mut EventCx| {
        v.state.read(crate::reader::ReadTarget { dir: dir.clone(), file: f.clone(), files: files.clone(), title_path: title_path.clone(), title: title.clone() });
        cx.notify();
    }))
}

fn actions(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: &Work) -> Element<Higanbana> {
    let files: &[FileEntry] = w.files.as_ref().and_then(|r| r.as_ref().ok()).map_or(&[], |f| f.as_slice());
    let dir = w.files_path.clone().unwrap_or_default();
    let progress = &v.state.player_data.progress;
    // Continuar de onde parou: o vídeo mais recente; visto, o seguinte.
    let videos = video_files(files);
    let mut last: Option<(usize, f64, bool, i64)> = None;
    for (i, f) in videos.iter().enumerate() {
        if let Some(p) = progress.get(&join_path(&dir, &f.name))
            && last.is_none_or(|l| p.updated_at >= l.3)
        {
            last = Some((i, p.position, p.watched, p.updated_at));
        }
    }
    let resume = match last {
        Some((i, at, false, _)) => Some((videos[i].clone(), at)),
        Some((i, _, true, _)) => videos.get(i + 1).cloned().map(|f| (f, 0.)),
        None => videos.first().cloned().map(|f| (f, 0.)),
    };
    let reading = v.state.core.reader().store.data().progress;
    let comics = comic_files(files);
    let mut best: Option<(usize, usize, bool, i64, Option<String>)> = None;
    for (i, f) in comics.iter().enumerate() {
        if let Some(p) = reading.get(&join_path(&dir, &f.name))
            && best.as_ref().is_none_or(|b| p.updated_at >= b.3)
        {
            best = Some((i, p.page, p.read, p.updated_at, p.unit_name.clone()));
        }
    }
    let resume_read = match best {
        None => comics.first().map(|f| (f.clone(), 0, false, None)),
        Some((i, page, false, _, unit)) => Some((comics[i].clone(), page, true, unit)),
        Some((i, _, true, _, _)) => comics.get(i + 1).map(|f| (f.clone(), 0, false, None)),
    };
    let mut row = div().flex_wrap().gap(8.0);
    let mut has_primary = false;
    if let (Some((f, at)), Some(play)) = (resume, play_action(w)) {
        has_primary = true;
        let ep = episode_number(&f.name).map(|n| format!("ep. {n}")).unwrap_or_default();
        let label = if at > 0. { format!("Continuar {ep} ({})", format_time(at)).replace("  ", " ") } else { format!("Assistir {ep}").trim().to_owned() };
        let b = Button::new("titulo-assistir", label).icon(icon("play").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| play(v, &f, cx));
        row = row.child(b.render(cx));
    }
    if let (Some((f, page, res, unit)), Some(read)) = (resume_read, read_action(w)) {
        let label = match (&unit, res) {
            (Some(u), true) => format!("Continuar {}", label_text(&chapter_label(u))),
            (_, true) => format!("Continuar {} (pág. {})", label_text(&chapter_label(&f.name)), page + 1),
            _ => format!("Ler {}", label_text(&chapter_label(&f.name))),
        };
        let b = Button::new("titulo-ler", label).variant(if has_primary { Variant::Outline } else { Variant::Default }).icon(icon("book-open").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| read(v, &f, cx));
        row = row.child(b.render(cx));
    }
    let paths = crate::ui::paths::file_paths(&dir, files);
    let remaining: Vec<String> = paths.iter().filter(|p| !matches!(v.state.downloads.get(p).map(|d| d.status), Some(DlStatus::Done | DlStatus::Running | DlStatus::Queued))).cloned().collect();
    let label = if !remaining.is_empty() {
        format!("Baixar {} ({})", if remaining.len() == paths.len() { "todos" } else { "restantes" }, remaining.len())
    } else if !paths.is_empty() {
        "Tudo baixado ou na fila".into()
    } else {
        "Nada para baixar".into()
    };
    let empty = remaining.is_empty();
    let dl = Button::new("titulo-baixar", label).variant(Variant::Outline).icon(icon("download").size(14.0)).disabled(empty).on_click(move |v: &mut Higanbana, _, cx| {
        v.state.start_downloads(&remaining);
        cx.notify();
    });
    // Fora de franquia abre o título inteiro: irmãs sem data (Filmes, Extras) ficam ao alcance.
    let raw = raw_path(&if w.model.franchise { w.model.season_path() } else { w.model.title_path.clone() });
    let folder = Button::new("titulo-pasta", "Ver pasta").variant(Variant::Ghost).icon(icon("folder").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
        v.state.go(raw.clone());
        cx.notify();
    });
    row.child(dl.render(cx)).child(folder.render(cx))
}

fn synopsis(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: &Work) -> Option<Element<Higanbana>> {
    let t = cx.theme().clone();
    let (paras, source) = anitext::clean_description(w.detail.as_ref()?.description.as_deref());
    if paras.is_empty() {
        return None;
    }
    let long = paras.join(" ").len() > 300;
    let expanded = v.pages.title().expanded;
    let mut text_col = div().flex_col().gap(12.0).text_color(t.foreground.fade(0.85));
    if long && !expanded {
        text_col = text_col.line_clamp(3).child(text(paras.join(" ")));
    } else {
        for p in paras {
            text_col = text_col.child(div().flex_col().child(text(p)));
        }
    }
    let mut col = div().max_w(672.0).flex_col().gap(8.0).child(text_col);
    if long || source.is_some() {
        let mut row = div().gap(12.0).items_center();
        if long {
            let b = Button::new("titulo-mais", if expanded { "Mostrar menos" } else { "Mostrar mais" }).variant(Variant::Link).size(ComponentSize::Xs).on_click(|v: &mut Higanbana, _, cx| {
                let ui = v.pages.title();
                ui.expanded = !ui.expanded;
                cx.notify();
            });
            row = row.child(b.render(cx));
        }
        if let Some(s) = source.filter(|_| expanded || !long) {
            row = row.child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(format!("Fonte: {s}"))));
        }
        col = col.child(row);
    }
    Some(col)
}

pub fn head(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: &Work) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let media = w.media.as_ref();
    let tmdb = w.episodes.clone().flatten();
    let rating = v.state.rating_source().work(media, tmdb.as_deref());
    let meta: Vec<String> = media
        .map(|m| {
            [
                rating.map(|s| format!("★ {}", s.label())),
                m.format.as_deref().map(|f| anitext::format(Some(f))),
                m.year().map(|y| y.to_string()),
                m.episodes.map(|e| format!("{e} {}", if e == 1 { "episódio" } else { "episódios" })),
                m.chapters.map(|c| format!("{c} capítulos")),
                w.detail.as_ref().and_then(|d| d.studios.nodes.first().map(|s| s.name.clone())),
            ]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect()
        })
        .unwrap_or_default();
    let native = media.and_then(|m| m.title.native.clone()).filter(|n| *n != w.heading);
    let english = media.and_then(|m| m.title.english.clone()).filter(|n| *n != w.heading);
    let genres = media.map(|m| m.genres.iter().map(|g| anitext::genre(g)).collect::<Vec<_>>().join(", ")).filter(|g| !g.is_empty());
    let mut info = div().flex_col().flex_1().min_w(0.0).gap(16.0);
    if w.model.franchise {
        let target = w.model.title_path.clone();
        let back = Button::new("titulo-franquia", folder_title(base_name(&w.model.title_path))).variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("chevron-left").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
            v.state.go(target.clone());
            cx.notify();
        });
        info = info.child(div().child(back.render(cx)));
    }
    let mut names = div().flex_col().gap(4.0);
    if let Some(n) = native {
        names = names.child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(n)));
    }
    names = names.child(div().flex_col().text_size(30.0).font_weight(700).child(text(w.heading.clone())));
    if let Some(n) = english {
        names = names.child(div().text_color(t.muted_foreground).child(text(n)));
    }
    info = info.child(names);
    if !meta.is_empty() || genres.is_some() {
        let mut m = div().flex_col().gap(4.0);
        if !meta.is_empty() {
            m = m.child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(meta.join(" · "))));
        }
        if let Some(g) = genres {
            m = m.child(div().text_size(t.typography.sm).child(text(g)));
        }
        info = info.child(m);
    }
    if let Some(s) = synopsis(v, cx, w) {
        info = info.child(s);
    }
    if matches!(w.summary, Some(None)) && w.model.title.is_some() {
        info = info.child(div().max_w(608.0).text_size(t.typography.sm).text_color(t.muted_foreground).child(text("Não achamos este título no AniList. Os arquivos continuam disponíveis abaixo.")));
    }
    info = info.child(actions(v, cx, w));
    let mut links = div().gap(16.0).text_size(t.typography.sm);
    if let Some(tr) = w.detail.as_ref().and_then(|d| d.trailer.clone()).filter(|t| t.site.as_deref() == Some("youtube")) {
        let url = format!("https://www.youtube.com/watch?v={}", tr.id);
        links = links.child(Link::new("titulo-trailer", "Trailer no YouTube").on_click(move |_: &mut Higanbana, _| open_url(&url)).render(cx));
    }
    if let Some(url) = w.detail.as_ref().and_then(|d| d.site_url.clone()) {
        let mal_only = w.detail.as_ref().is_some_and(|d| d.summary.is_mal_only());
        links = links.child(Link::new("titulo-site", if mal_only { "Ver no MyAnimeList" } else { "Página no AniList" }).on_click(move |_: &mut Higanbana, _| open_url(&url)).render(cx));
    }
    info = info.child(links);
    let art = div().flex_shrink_0().rounded(t.radius_lg).shadow(Color::BLACK.fade(0.25), SHADOW_LG).child(cover(v, cx, media, &w.model.lookup_name(), Some(184.0)));
    div().mt(48.0).items_start().gap(32.0).child(art).child(info)
}
