//! Cabeçalho do catálogo: migalhas, título, busca + ordenação + coleções, barra
//! de filtros, linha de status e as obras achadas dentro de franquias.

use higanbana_core::library::join_path;
use higanbana_gpui::components::{Button, Component, DropdownMenu, Input, PopoverAnchor, Variant};
use higanbana_gpui::prelude::*;

use super::filters::bar::filter_bar;
use super::filters::logic::{anilist_pause_line, sort_label, status_line, SORTS};
use super::render_row::{Ctx, GAP_X};
use super::{restart, ui};
use crate::root::Higanbana;
use crate::ui::cover::title_card;
use crate::ui::paths::breadcrumbs;

fn caret_button(cx: &mut RenderCx<'_, Higanbana>, id: &'static str, label: String, disabled: bool) -> Element<Higanbana> {
    let b = Button::new(id, label).variant(Variant::Outline).size(higanbana_gpui::components::ComponentSize::Sm).trailing_icon(icon("chevron-down").size(14.0)).disabled(disabled);
    b.render(cx)
}

fn sort_menu(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let c = ui(v);
    let (sort, state) = (c.sort, c.menus.sort);
    let trigger = caret_button(cx, "catalogo-ordenar", format!("Ordenar: {}", sort_label(sort)), false);
    let mut menu = DropdownMenu::new("catalogo-ordem", &state, |v: &mut Higanbana| &mut ui(v).menus.sort, trigger).anchor(PopoverAnchor::TopRight);
    for s in SORTS {
        menu = menu.item(sort_label(s), s == sort, move |v: &mut Higanbana, cx| {
            if ui(v).sort != s {
                ui(v).sort = s;
                restart(v, cx);
            }
        });
    }
    menu.render(cx)
}

fn collections_menu(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let c = ui(v);
    let (media, root, state) = (c.series.media, c.series.root, c.menus.collections);
    let names = v.state.catalog_collections(media);
    let trigger = caret_button(cx, "catalogo-colecoes", "Coleções".into(), names.is_empty());
    if names.is_empty() {
        return trigger;
    }
    let mut menu = DropdownMenu::new("catalogo-colecao", &state, |v: &mut Higanbana| &mut ui(v).menus.collections, trigger).anchor(PopoverAnchor::TopRight).max_height(420.0);
    for name in names {
        let target = join_path(root, &name);
        menu = menu.item(name, false, move |v: &mut Higanbana, _| v.state.go(target.clone()));
    }
    menu.render(cx)
}

pub fn header(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, ctx: &Ctx) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let c = ui(v);
    let (series, query) = (c.series, c.query());
    let media = series.media;
    let search = Input::new("catalogo-busca", &c.search, |v: &mut Higanbana| &mut ui(v).search).kit().prefix("search").on_change(|v: &mut Higanbana, _, cx| restart(v, cx)).render(cx);
    let (indexed, total) = v.state.index_progress(media);
    let mut col = div()
        .id("catalogo-cabecalho")
        .track_bounds()
        .flex_col()
        .pt(24.0)
        .gap(16.0)
        .child(breadcrumbs(cx, series.root, false))
        .child(text(series.title).text_size(t.typography.xxl).font_weight(600))
        .child(div().gap(8.0).child(div().flex_1().min_w(0.0).child(search)).child(sort_menu(v, cx)).child(collections_menu(v, cx)))
        .child(filter_bar(v, cx));
    if let Some((works, (resolved, all))) = ctx.flat_status {
        let pending = resolved < all || indexed < total;
        let mut line = div().items_center().gap(8.0).text_size(t.typography.xs).text_color(t.muted_foreground);
        if pending {
            line = line.child(div().w(6.0).h(6.0).rounded_full().bg(t.primary));
        }
        line = line.child(text(status_line(works, (resolved, all), (indexed, total))));
        if let Some(left) = ctx.anilist_left {
            line = line.child(text(format!("· {}", anilist_pause_line(left))));
        }
        col = col.child(line);
    } else if !query.is_empty() && indexed < total {
        col = col.child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(format!("Indexando nomes… {indexed}/{total}"))));
    }
    if !ctx.franchise.is_empty() {
        let mut grid = div().flex_wrap().gap(GAP_X).pb(8.0);
        for r in &ctx.franchise {
            grid = grid.child(title_card(v, cx, "franchise", &r.name, &r.path, media, ctx.m.card_w));
        }
        let head = div().items_center().gap(12.0).child(text("Dentro de franquias").text_size(t.typography.lg).font_weight(600)).child(div().flex_1().h(1.0).bg(t.border));
        col = col.child(div().flex_col().pt(16.0).gap(16.0).child(head).child(grid));
    }
    if v.state.catalog_cloud(media).is_some_and(|s| s.failed && s.query == query) {
        col = col.child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text("Busca na nuvem indisponível agora")));
    }
    if v.state.cloud_paused_until().is_some() {
        col = col.child(div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text("A nuvem pediu uma pausa; retomando em instantes")));
    }
    col
}
