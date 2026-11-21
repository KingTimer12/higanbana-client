//! Barra de filtros: botões que abrem menus (Formato, Gênero, Ano, Temporada),
//! Dublado, chips removíveis e "Limpar".

use higanbana_core::anime_index::LETTERS;
use higanbana_core::catalog_filter::Filters;
use higanbana_gpui::components::{Button, Component, ComponentSize, Input, Popover, PopoverState, Variant};
use higanbana_gpui::prelude::*;

use super::logic::{genres_to_offer, kind_label, loaded_genres, parse_year, season_label, toggle, trigger_label, year_label, year_range, KINDS, SEASONS};
use crate::root::Higanbana;
use crate::views::catalog::{restart, ui, CatalogUi};

type Lens = fn(&mut CatalogUi) -> &mut PopoverState;

/// Botão pílula (opção de menu ou chip).
fn pill(cx: &mut RenderCx<'_, Higanbana>, id: ElementIdish, label: String, on: bool, f: impl Fn(&mut Higanbana, &mut EventCx) + 'static) -> Element<Higanbana> {
    let accent = cx.theme().accent;
    let b = Button::new(id.0, label).variant(Variant::Outline).size(ComponentSize::Compact).on_click(move |v: &mut Higanbana, _, cx| {
        f(v, cx);
        restart(v, cx);
    });
    let b = b.render(cx).rounded_full();
    if on { b.bg(accent) } else { b }
}

/// Id composto (prefixo, rótulo).
struct ElementIdish(higanbana_gpui::ElementId);

fn id(prefix: &str, label: &str) -> ElementIdish {
    ElementIdish(higanbana_gpui::ElementId::from((prefix, label)))
}

/// Opções em chips que ligam e desligam sem fechar o menu.
fn options<T: Clone + PartialEq + 'static>(cx: &mut RenderCx<'_, Higanbana>, prefix: &'static str, items: Vec<(T, String)>, chosen: &[T], pick: fn(&mut Filters) -> &mut Vec<T>) -> Element<Higanbana> {
    let mut row = div().flex_wrap().gap(6.0).max_w(360.0);
    for (x, label) in items {
        let on = chosen.contains(&x);
        row = row.child(pill(cx, id(prefix, &label), label, on, move |v, _| toggle(pick(&mut ui(v).filters), x.clone())));
    }
    row
}

fn menu_button(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, lens: Lens, name: &'static str, count: usize, content: impl FnOnce(&mut Higanbana, &mut RenderCx<'_, Higanbana>) -> Element<Higanbana>) -> Element<Higanbana> {
    let state = *lens(ui(v));
    let accent = cx.theme().accent;
    let trigger = Button::new(("filtro", name), trigger_label(name, count)).variant(Variant::Outline).size(ComponentSize::Compact).trailing_icon(icon("chevron-down").size(12.0)).render(cx);
    let trigger = if count > 0 { trigger.bg(accent) } else { trigger };
    let body = if state.open { content(v, cx) } else { div() };
    Popover::new(("filtro-menu", name), &state, move |v: &mut Higanbana| lens(ui(v)), trigger).content(move |_| body).render(cx)
}

fn year_fields(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let read = |v: &mut Higanbana, _: &str, cx: &mut EventCx| {
        let c = ui(v);
        let years = year_range(parse_year(c.year_from.text()), parse_year(c.year_to.text()));
        if years != c.filters.years {
            c.filters.years = years;
            restart(v, cx);
        }
    };
    let c = ui(v);
    let from = Input::new("ano-de", &c.year_from, |v: &mut Higanbana| &mut ui(v).year_from).width(88.0).height(32.0).on_change(read).render(cx);
    let c = ui(v);
    let to = Input::new("ano-ate", &c.year_to, |v: &mut Higanbana| &mut ui(v).year_to).width(88.0).height(32.0).on_change(read).render(cx);
    div().items_center().gap(8.0).child(from).child(text("–")).child(to)
}

/// Gêneros dos metadados já carregados; refeito só quando chegam metadados.
fn offered_genres(v: &mut Higanbana) -> Vec<String> {
    let media = v.pages.catalog_media();
    let progress = v.state.metadata_progress(media);
    if v.pages.catalog(&v.state, media).genres.as_ref().is_none_or(|(k, _)| *k != progress) {
        let letters: Vec<_> = LETTERS.iter().filter_map(|l| v.state.catalog_letter(media, l)).collect();
        let metas = letters.iter().flat_map(|l| l.iter()).filter_map(|e| v.state.lookup_ref(&e.name, media).flatten());
        let g = loaded_genres(metas);
        v.pages.catalog(&v.state, media).genres = Some((progress, g));
    }
    let c = v.pages.catalog(&v.state, media);
    genres_to_offer(c.genres.as_ref().map_or(&[][..], |(_, g)| g), &c.filters.genres)
}

/// Chips dos filtros ativos; clicar tira o filtro.
fn chips(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Vec<Element<Higanbana>> {
    let f = ui(v).filters.clone();
    let mut out = Vec::new();
    let mut chip = |cx: &mut RenderCx<'_, Higanbana>, key: String, label: String, remove: Box<dyn Fn(&mut CatalogUi)>| {
        let b = Button::new(("chip", key.as_str()), label).variant(Variant::Outline).size(ComponentSize::Compact).icon(icon("x").size(12.0)).on_click(move |v: &mut Higanbana, _, cx| {
            remove(ui(v));
            restart(v, cx);
        });
        let accent = cx.theme().accent;
        out.push(b.render(cx).rounded_full().bg(accent));
    };
    for k in f.kinds {
        chip(cx, format!("k{}", kind_label(k)), kind_label(k).into(), Box::new(move |c| c.filters.kinds.retain(|x| *x != k)));
    }
    for g in f.genres {
        let name = g.clone();
        chip(cx, format!("g{g}"), g, Box::new(move |c| c.filters.genres.retain(|x| *x != name)));
    }
    if let Some(years) = f.years {
        chip(cx, "anos".into(), year_label(years), Box::new(|c| {
            c.year_from.set_text("");
            c.year_to.set_text("");
            c.filters.years = None;
        }));
    }
    for s in f.seasons {
        chip(cx, format!("s{}", season_label(s)), season_label(s).into(), Box::new(move |c| c.filters.seasons.retain(|x| *x != s)));
    }
    if f.dubbed {
        chip(cx, "dub".into(), "Dublado".into(), Box::new(|c| c.filters.dubbed = false));
    }
    out
}

pub fn filter_bar(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let c = ui(v);
    let (series, f) = (c.series, c.filters.clone());
    let mut bar = div().flex_wrap().items_center().gap(8.0);
    if series.kind_filter {
        let chosen = f.kinds.clone();
        bar = bar.child(menu_button(v, cx, |c| &mut c.menus.kind, "Formato", f.kinds.len(), move |_, cx| options(cx, "formato", KINDS.map(|k| (k, kind_label(k).to_owned())).to_vec(), &chosen, |f| &mut f.kinds)));
    }
    let chosen = f.genres.clone();
    bar = bar.child(menu_button(v, cx, |c| &mut c.menus.genre, "Gênero", f.genres.len(), move |v, cx| {
        let offered = offered_genres(v);
        if offered.is_empty() {
            let muted = cx.theme().muted_foreground;
            return div().max_w(280.0).text_size(cx.theme().typography.sm).text_color(muted).child(text("Os gêneros aparecem conforme os metadados das obras chegam."));
        }
        options(cx, "genero", offered.into_iter().map(|g| (g.clone(), g)).collect(), &chosen, |f| &mut f.genres)
    }));
    bar = bar.child(menu_button(v, cx, |c| &mut c.menus.year, "Ano", usize::from(f.years.is_some()), year_fields));
    if series.season_filter {
        let chosen = f.seasons.clone();
        bar = bar.child(menu_button(v, cx, |c| &mut c.menus.season, "Temporada", f.seasons.len(), move |_, cx| options(cx, "temporada", SEASONS.map(|s| (s, season_label(s).to_owned())).to_vec(), &chosen, |f| &mut f.seasons)));
    }
    if series.dubbed_filter {
        bar = bar.child(pill(cx, id("filtro", "dublado"), "Dublado".into(), f.dubbed, |v, _| {
            let c = ui(v);
            c.filters.dubbed = !c.filters.dubbed;
        }));
    }
    if !f.is_empty() {
        bar = bar.child(div().w(1.0).h(20.0).mx(4.0).bg(t.border));
        for c in chips(v, cx) {
            bar = bar.child(c);
        }
        let clear = Button::new("filtro-limpar", "Limpar").variant(Variant::Ghost).size(ComponentSize::Compact).on_click(|v: &mut Higanbana, _, cx| {
            let c = ui(v);
            c.filters = Filters::default();
            c.year_from.set_text("");
            c.year_to.set_text("");
            restart(v, cx);
        });
        bar = bar.child(clear.render(cx));
    }
    bar
}
