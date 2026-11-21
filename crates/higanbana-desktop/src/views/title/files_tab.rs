//! Aba de arquivos: temporadas, versões (cartões), extras e a lista de arquivos.

use higanbana_core::library::{folder_title, folder_year, join_path, release_tags, LOOSE_FILES_VERSION};
use higanbana_core::player::logic::video_files;
use higanbana_gpui::components::{Button, Component, ComponentSize, Skeleton, Variant};
use higanbana_gpui::prelude::*;

use super::data::label_of;
use super::header::{play_action, read_action};
use super::Work;
use crate::root::Higanbana;
use crate::ui::files::{file_list, sorted, FileRows};
use crate::ui::options::{option_button, option_group, version_card};
use crate::ui::empty_state;

pub fn files_tab(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: &Work, width: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let m = &w.model;
    let seasons = m.seasons();
    let releases = m.releases();
    let season_name = m.season_name();
    let work_path = m.season_path();
    let work_title = m.work_title();
    let selected = w.release.clone().filter(|_| m.extra_name().is_none());
    let mut options = div().flex_col().gap(12.0);
    // Franquias trocam de obra pela página da franquia.
    if seasons.len() > 1 && !m.franchise {
        let mut row = div().flex_wrap().gap(8.0);
        for s in &seasons {
            let target = join_path(&m.title_path, &s.name);
            let year = folder_year(&s.name).map(|y| format!("{y} · ")).unwrap_or_default();
            row = row.child(option_button(cx, ("temporada", s.name.as_str()), format!("{year}{}", folder_title(&s.name)), Some(&s.name) == season_name.as_ref(), move |v, _| v.state.go(target.clone())));
        }
        options = options.child(option_group(&t, "Temporada").child(row));
    }
    if releases.len() > 1 {
        // Só a versão aberta foi listada: as outras ficam sem contagem.
        let count = w.files.as_ref().and_then(|r| r.as_ref().ok()).map(|f| video_files(f).len()).filter(|&n| n > 0);
        let mut row = div().flex_wrap().gap(8.0);
        for r in &releases {
            let on = Some(&r.name) == selected.as_ref();
            let target = if r.name == LOOSE_FILES_VERSION { work_path.clone() } else { join_path(&work_path, &r.name) };
            let (work, folder) = (work_path.clone(), r.name.clone());
            let card = version_card(cx, ("versao", r.name.as_str()), &label_of(&r.name, &work_title), count.filter(|_| on), on).on_click(move |v: &mut Higanbana, _, cx| {
                v.state.core.prefs().set_version(&work, &folder);
                v.state.go(target.clone());
                cx.notify();
            });
            row = row.child(card);
        }
        options = options.child(option_group(&t, "Versão").child(row));
    }
    if releases.len() == 1 {
        let tags = release_tags(&releases[0].name);
        if !tags.is_empty() {
            options = options.child(option_group(&t, "Versão").child(div().pt(4.0).text_size(t.typography.sm).text_color(t.muted_foreground).child(text(tags.join(" ")))));
        }
    }
    let body = match &w.files {
        Some(Err(e)) => empty_state(&t, "wifi-off", "Não foi possível abrir os arquivos", Some(e.clone().into())),
        None => Skeleton::new("titulo-arquivos").w(pct(100.0)).h(288.0).render(cx).rounded(t.radius_lg),
        Some(Ok(list)) => {
            let dir = w.files_path.clone().unwrap_or_default();
            let rows = FileRows::new(&dir, sorted(list), false, play_action(w), read_action(w));
            let _ = width;
            file_list(v, cx, &rows)
        }
    };
    let mut col = div().flex_col().gap(20.0).child(options).child(body);
    let extras = &m.work_split.1;
    if !extras.is_empty() {
        let mut row = div().flex_wrap().gap(8.0);
        for e in extras {
            let target = join_path(&work_path, &e.name);
            let b = Button::new(("extra", e.name.as_str()), folder_title(&e.name)).variant(Variant::Outline).size(ComponentSize::Compact).icon(icon("folder").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
                v.state.go(target.clone());
                cx.notify();
            });
            row = row.child(b.render(cx));
        }
        col = col.child(div().flex_col().gap(8.0).child(text("Extras").text_size(t.typography.sm).font_weight(600)).child(row));
    }
    col
}
