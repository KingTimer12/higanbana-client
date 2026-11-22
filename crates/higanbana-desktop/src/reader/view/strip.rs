//! Faixa na tela: long strip em lista virtual (só o visível é montado) e wide strip
//! em rolagem horizontal. A posição de leitura vem do item no centro da área.

use std::sync::Arc;

use higanbana_core::reader::logic::{chapter_label, label_text, ReadMode};
use higanbana_gpui::components::{item_offset, visible_range, Component, Spinner, VirtualList};
use higanbana_gpui::prelude::*;

use super::{end, page};
use crate::reader::state::page_key;
use crate::reader::strip::{StripItem, StripKind};
use crate::root::Higanbana;

pub const LONG: &str = "leitor-faixa-longa";
pub const WIDE: &str = "leitor-faixa-larga";
/// Largura máxima das páginas no long strip (50 rem).
const LONG_W: f32 = 800.0;

/// Tamanho de cada item no eixo da rolagem.
fn extents(v: &Higanbana, items: &[StripItem], long: bool, w: f32, h: f32) -> Vec<f32> {
    let r = v.state.reader.as_ref().expect("faixa só com leitor");
    items
        .iter()
        .map(|i| {
            let ratio = |k: &str| r.ratios.get(k).copied().unwrap_or(0.7);
            match (&i.kind, long) {
                (StripKind::Page { unit, page, .. }, true) => w / ratio(&page_key(unit, *page)),
                (StripKind::Page { unit, page, .. }, false) => {
                    if r.pages.get(&page_key(unit, *page)).is_some_and(|p| matches!(p, crate::state::Img::Ready(_))) {
                        h * ratio(&page_key(unit, *page))
                    } else {
                        page::PLACEHOLDER_W
                    }
                }
                (StripKind::End(_), true) => 320.0,
                (StripKind::End(_), false) => 320.0,
                (StripKind::Seam(_), true) => 120.0,
                (StripKind::Seam(_), false) => 160.0,
                (StripKind::Loading, _) => 120.0,
                (StripKind::Failed(..), _) => if long { 320.0 } else { 420.0 },
            }
        })
        .collect()
}

/// Item no centro da área (`offset` + metade da vista).
fn center_item(sizes: &[f32], offset: f32, view: f32) -> usize {
    let mid = offset + view / 2.0;
    let mut y = 0.0;
    for (i, s) in sizes.iter().enumerate() {
        y += s;
        if y > mid {
            return i;
        }
    }
    sizes.len().saturating_sub(1)
}

fn item(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, it: &StripItem, long: bool, w: f32, h: f32, size: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let label = |ch: usize| {
        let r = v.state.reader.as_ref().expect("faixa só com leitor");
        label_text(&chapter_label(&r.chapters.get(ch).map(|u| u.name.clone()).unwrap_or_default()))
    };
    let boxed = |e: Element<Higanbana>| if long { div().w(w).h(size).flex_col().items_center().justify_center().child(e) } else { div().w(size).h(h).flex_col().items_center().justify_center().child(e) };
    match &it.kind {
        StripKind::Page { unit, page, .. } => {
            if long {
                page::page_image(v, cx, unit, *page, w, None)
            } else {
                page::page_image(v, cx, unit, *page, size, Some(h))
            }
        }
        StripKind::Loading => boxed(Spinner::new(("leitor-carregando", it.key.0.as_str())).render(cx)),
        StripKind::End(ch) => boxed(end::chapter_end(v, cx, *ch)),
        StripKind::Seam(ch) => boxed(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(label(ch + 1)))),
        StripKind::Failed(_, msg) => boxed(end::open_failed(v, cx, msg.clone())),
    }
}

/// Monta a faixa e atualiza posição, janela de páginas e emenda.
pub fn view(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, area_w: f32, area_h: f32) -> Element<Higanbana> {
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let long = r.prefs.mode == ReadMode::Long;
    let id = if long { LONG } else { WIDE };
    let items = r.strip_items();
    let (w, h) = (LONG_W.min(area_w - 32.0).max(1.0), (area_h - 44.0).max(1.0));
    let sizes = extents(v, &items, long, w, h);
    let view_len = if long { area_h } else { area_w };
    let offset = cx.scroll_offset(id);
    // Pedido de ir a um item (abrir capítulo, painel de páginas, troca de modo).
    let pending = r.pending_scroll.clone();
    let mut changed = false;
    if let Some(target) = pending
        && let Some(ix) = items.iter().position(|i| i.key == target)
    {
        cx.scroll_to(id, item_offset(&sizes, 0.0, ix));
        let r = v.state.reader.as_mut().expect("checado");
        r.pending_scroll = None;
        changed |= r.on_strip_top(&items, ix);
    } else if !items.is_empty() {
        let top = center_item(&sizes, offset, view_len);
        let r = v.state.reader.as_mut().expect("checado");
        changed |= r.on_strip_top(&items, top);
    }
    let vis = visible_range(&sizes, 0.0, offset, view_len);
    let total: f32 = sizes.iter().sum();
    let r = v.state.reader.as_mut().expect("checado");
    r.strip_scroll = (offset, view_len);
    r.set_strip_window(&items, vis.start..vis.end);
    if total - offset < view_len * 2.0 {
        changed |= r.extend_strip();
    }
    // Pede o que faltar (uma vez por página: `request_pages` ignora o já pedido).
    let missing = r.strip_wants.iter().any(|w| r.pages.get(&w.key).is_none()) || !r.strip_opens().is_empty();
    if changed || missing {
        v.state.pump_reader();
    }
    if long {
        let heights: Arc<[f32]> = sizes.clone().into();
        let body = VirtualList::new(id, heights).render(cx, |i, cx| div().w_full().flex_col().items_center().child(item(v, cx, &items[i], true, w, h, sizes[i])));
        return div().size_full().child(body);
    }
    // Wide: só os itens perto da vista têm conteúdo; os outros guardam o lugar.
    let mut row = scroll_x(id).rerender_on_scroll().size_full().items_center();
    for (i, it) in items.iter().enumerate() {
        let near = i + 1 >= vis.start && i <= vis.end;
        let cell = if near { item(v, cx, it, false, w, h, sizes[i]) } else { div().w(sizes[i]).h(h) };
        row = row.child(cell.flex_shrink_0());
    }
    row
}
