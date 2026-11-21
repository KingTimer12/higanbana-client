//! Cartões do "Continuar": o destaque (capa, título, progresso e botão) e os
//! cartões das fileiras. Abrir lista só a pasta do arquivo, no clique.

use higanbana_core::continue_list::ContinueItem;
use higanbana_core::library::{base_name, folder_title, join_path, parent_path};
use higanbana_core::player::logic::video_files;
use higanbana_gpui::components::{Button, Component, ComponentSize, Progress, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};
use higanbana_gpui::{Corners, ElementId};

use super::continue_row::{item_label, media_of, CARD_W};
use crate::motion::rise;
use crate::root::Higanbana;
use crate::state::{AppState, Tone};
use crate::ui::cover::{cover, want_lookup};
use crate::ui::image::hex_color;
use crate::ui::{hover, SHADOW_LG};
use crate::video::PlayTarget;

pub const CARD_H: f32 = 80.0;
const FEATURED_H: f32 = 144.0;

/// Barra que enche com mola ao aparecer (rigidez 120, amortecimento 20).
fn filling_progress(cx: &mut RenderCx<'_, Higanbana>, key: &str, fraction: f32) -> Element<Higanbana> {
    let id = ElementId::from(("continuar-progresso", key));
    let tr = Transition::spring(120.0, 20.0);
    let v = cx.animate_from(id, 0.0, fraction.clamp(0.0, 1.0), tr);
    Progress::new(id).value(v * 100.0).render(cx)
}

fn title(s: &AppState, item: &ContinueItem) -> String {
    s.lookup(base_name(&item.work_path), media_of(&item.work_path)).flatten().map_or_else(|| folder_title(base_name(&item.work_path)), |m| m.title())
}

/// Retoma: lista a pasta do arquivo (só neste clique) e abre o player. Visto até o fim abre a pasta.
pub fn open(s: &mut AppState, item: &ContinueItem, is_reading: bool) {
    let dir = parent_path(&item.file_path);
    if is_reading || item.done {
        // O leitor volta no Plano 5; até lá, a pasta do capítulo.
        s.go(dir);
        return;
    }
    let media = s.lookup(base_name(&item.work_path), media_of(&item.work_path)).flatten();
    let anime_title = title(s, item);
    let (file_path, core, list_dir) = (item.file_path.clone(), s.core.clone(), dir.clone());
    s.run(async move { core.cloud().list(&list_dir).await.map(|l| l.files).map_err(|e| e.to_string()) }, move |s, listed, cx| {
        let files = match listed {
            Ok(f) => f,
            Err(e) => return s.toast(format!("Não foi possível abrir a pasta: {e}"), Tone::Error),
        };
        let Some(file) = files.iter().find(|f| join_path(&dir, &f.name) == file_path).cloned() else {
            return s.toast("O arquivo não está mais na nuvem.", Tone::Error);
        };
        let mal_id = media.and_then(|m| m.id_mal).and_then(|v| u32::try_from(v).ok());
        s.play(PlayTarget { files: video_files(&files), file, is_anime: dir.starts_with("Animes/"), mal_id, anime_title, dir });
        cx.notify();
    });
}

fn open_click(item: &ContinueItem, is_reading: bool) -> impl Fn(&mut Higanbana, &MouseEvent, &mut EventCx) + 'static {
    let item = item.clone();
    move |v, _, cx| {
        open(&mut v.state, &item, is_reading);
        cx.notify();
    }
}

pub fn featured(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, item: &ContinueItem, is_reading: bool) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let (name, media) = (base_name(&item.work_path).to_owned(), media_of(&item.work_path));
    let looked = v.state.lookup(&name, media);
    let m = looked.clone().flatten();
    let title = title(&v.state, item);
    let label = item_label(base_name(&item.file_path), item, is_reading);
    let (action, ic) = match (is_reading, item.done) {
        (true, _) => ("Continuar lendo", "book-open"),
        (false, false) => ("Continuar", "play"),
        (false, true) => ("Ver episódios", "chevron-right"),
    };
    let glow = hex_color(m.as_ref().and_then(|m| m.color())).unwrap_or(t.primary);
    let light = div().absolute().top(0.0).left(0.0).size_full().bg_gradient(LinearGradient::new(GradientDir::Right, vec![(0.0, glow.alpha(0.3)), (0.65, glow.alpha(0.0))]));
    let art = div().flex_shrink_0().rounded(t.radius).overflow_hidden().shadow(Color::BLACK.alpha(0.25), SHADOW_LG).child(cover(v, cx, m.as_deref(), &title, Some(76.0)));
    let info = div()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .gap(6.0)
        .child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(if is_reading { "Último capítulo aberto" } else { "Último episódio" })))
        .child(div().flex_col().text_size(t.typography.xl).font_weight(600).line_clamp(1).child(text(title)))
        .child(div().flex_col().text_size(t.typography.sm).text_color(t.muted_foreground).line_clamp(1).child(text(label)))
        .child(div().pt(4.0).max_w(384.0).child(filling_progress(cx, &item.file_path, item.fraction)));
    let btn = Button::new("continuar-abrir", action).variant(Variant::Default).size(ComponentSize::Sm).icon(icon(ic).size(14.0)).on_click(open_click(item, is_reading)).render(cx);
    let mut d = div().relative().h(FEATURED_H).px(16.0).gap(20.0).items_center().rounded(t.radius_lg).border(1.0, t.border).bg(t.group_box).overflow_hidden().child(light).child(art).child(info).child(btn);
    if looked.is_none() {
        d = d.child(want_lookup(name, media));
    }
    d
}

pub fn card(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, item: &ContinueItem, is_reading: bool, i: usize) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let (name, media) = (base_name(&item.work_path).to_owned(), media_of(&item.work_path));
    let looked = v.state.lookup(&name, media);
    let m = looked.clone().flatten();
    let title = title(&v.state, item);
    let label = item_label(base_name(&item.file_path), item, is_reading);
    let id = ElementId::from(("continuar-cartao", item.work_path.as_str()));
    let ring = hover::layer(hover::amount(cx, id), Corners::all(t.radius_lg), None, Some(t.ring));
    let enter = rise(cx, ("continuar-entra", item.work_path.as_str()), i);
    let info = div()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .gap(4.0)
        .child(div().flex_col().text_size(t.typography.sm).font_weight(600).line_clamp(1).child(text(title.clone())))
        .child(div().flex_col().text_size(t.typography.xs).text_color(t.muted_foreground).line_clamp(1).child(text(label)))
        .child(div().pt(4.0).child(filling_progress(cx, &format!("cartao:{}", item.file_path), item.fraction)));
    let mut d = div()
        .relative()
        .w(CARD_W)
        .h(CARD_H)
        .pr(12.0)
        .gap(12.0)
        .items_center()
        .rounded(t.radius_lg)
        .border(1.0, t.border)
        .bg(t.group_box)
        .overflow_hidden()
        .cursor_pointer()
        .opacity(enter)
        .child(div().flex_shrink_0().h_full().child(cover(v, cx, m.as_deref(), &title, Some(CARD_H / 1.5))))
        .child(info)
        .child(ring)
        .on_click(open_click(item, is_reading));
    if looked.is_none() {
        d = d.child(want_lookup(name, media));
    }
    hover::track(d, id)
}
