//! Paleta de busca (Ctrl K): `Command` num diálogo sem título, 40 rem de largura.

use higanbana_core::library::{media_for_kind, split_path};
use higanbana_core::nuvem::SearchResult;
use higanbana_gpui::components::{Command, CommandGroup, CommandItem, Component, Dialog};
use higanbana_gpui::prelude::*;

use crate::root::Higanbana;
use crate::state::{kind_heading, Img};
use crate::ui::{empty_state, truncate};

const PLACEHOLDER: &str = "Buscar animes, mangás, OSTs…";

fn thumb(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, r: &SearchResult) -> Element<Higanbana> {
    let t = cx.theme();
    let url = media_for_kind(r.kind).and_then(|m| v.state.lookup(&r.name, m).flatten()).and_then(|m| m.cover_small().map(str::to_owned));
    let image = url.map(|u| v.state.image(&u));
    let frame = div().w(32.0).h(48.0).flex_shrink_0().items_center().justify_center().overflow_hidden().rounded(t.radius).bg(t.muted);
    match image {
        Some(Img::Ready(src)) => frame.child(img(src).size_full().rounded(t.radius)),
        _ => frame.child(icon("folder").size(14.0).text_color(t.muted_foreground)),
    }
}

fn item(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, g: usize, i: usize, r: &SearchResult) -> CommandItem<Higanbana> {
    let parent = split_path(&r.path).iter().rev().skip(1).rev().copied().collect::<Vec<_>>().join(" / ");
    let t = cx.theme();
    let (muted, xs) = (t.muted_foreground, t.typography.xs);
    let thumb = thumb(v, cx, r);
    let text_col = div()
        .flex_col()
        .flex_1()
        .min_w(0.0)
        .child(truncate(r.name.clone()).font_weight(500))
        .child(truncate(parent).text_size(xs).text_color(muted));
    CommandItem::new(("busca", g * 1000 + i)).child(div().w_full().items_center().gap(12.0).py(4.0).child(thumb).child(text_col))
}

/// Diálogo da paleta (nada se fechada).
pub fn palette(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let groups = v.state.search.groups.clone();
    let mut command_groups = Vec::new();
    for (g, (kind, items)) in groups.iter().enumerate() {
        let mut group = CommandGroup::new(kind_heading(*kind));
        for (i, r) in items.iter().enumerate() {
            group = group.item(item(v, cx, g, i, r));
        }
        command_groups.push(group);
    }
    let q = v.state.search.query.clone();
    let empty = if q.is_empty() {
        empty_state(cx.theme(), "search", "Busque pelo nome da pasta", Some("Funciona melhor com o título em romaji, como “Shingeki no Kyojin”.".into()))
    } else {
        empty_state(cx.theme(), "search", format!("Nada encontrado para “{q}”"), Some("Tente o título em romaji ou uma parte menor do nome.".into()))
    };
    let mut command = Command::new("paleta", &v.state.search.cmd, |v: &mut Higanbana| &mut v.state.search.cmd)
        .placeholder(PLACEHOLDER)
        // Kit: `rems(28.)`.
        .max_h(448.0)
        .empty(empty)
        .on_query(|v: &mut Higanbana, q, cx| {
            v.state.on_search_query(q);
            cx.notify();
        })
        .on_confirm(|v: &mut Higanbana, ix, cx| {
            v.state.confirm_search(ix);
            cx.notify();
        });
    for g in command_groups {
        command = command.group(g);
    }
    let body = command.render(cx);
    Dialog::new("paleta-dialogo", v.state.search.open)
        .width(640.0)
        .child(body)
        .on_open_change(|v: &mut Higanbana, open, cx| {
            if !open {
                v.state.close_search();
            }
            cx.notify();
        })
        .render(cx)
}

/// Id do campo da paleta (foco ao abrir).
pub fn input_id() -> higanbana_gpui::ElementId {
    Command::<Higanbana>::input_id("paleta")
}
