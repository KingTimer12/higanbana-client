//! Desenho de cada linha da lista virtual do catálogo.

use std::time::Duration;

use higanbana_core::anime_index::LETTERS;
use higanbana_core::nuvem::SearchResult;
use higanbana_gpui::components::{Button, Component, Skeleton, Variant};
use higanbana_gpui::prelude::*;

use super::filters::logic::EmptyNotice;
use super::header::header;
use super::rows::{header_label, Metrics, Row, LETTER_HEAD, ROW_GAP};
use super::{restart, ui};
use crate::root::Higanbana;
use crate::ui::cover::title_card;
use crate::ui::empty_state;

pub struct Ctx {
    pub m: Metrics,
    pub width: f32,
    pub flat_status: Option<(usize, (usize, usize))>,
    pub anilist_left: Option<Duration>,
    pub franchise: Vec<SearchResult>,
    pub notice: Option<EmptyNotice>,
}

/// Coluna centralizada na largura do conteúdo.
fn centered(width: f32, child: Element<Higanbana>) -> Element<Higanbana> {
    div().w_full().flex_col().items_center().child(div().w(width).flex_col().child(child))
}

fn letter_head(t: &Theme, letter: usize, count: Option<usize>) -> Element<Higanbana> {
    let mut label = div().items_end().gap(8.0).child(text(header_label(LETTERS[letter]).to_owned()).text_size(t.typography.lg).font_weight(600));
    if let Some(n) = count {
        label = label.child(div().pb(2.0).text_size(t.typography.sm).text_color(t.muted_foreground).child(text(format!("· {n}"))));
    }
    div().h(LETTER_HEAD).pt(32.0).items_center().gap(12.0).child(label).child(div().flex_1().h(1.0).bg(t.border))
}

fn notice(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, ctx: &Ctx) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let media = v.pages.catalog_media();
    let (indexed, total) = v.state.index_progress(media);
    match ctx.notice {
        Some(EmptyNotice::NoFilterMatch) => {
            let pending = ctx.flat_status.is_some_and(|(_, (r, all))| r < all) || indexed < total;
            let detail = pending.then(|| "Ainda carregando o acervo; mais obras podem aparecer.".into());
            let clear = Button::new("catalogo-limpar", "Limpar filtros").variant(Variant::Outline).on_click(|v: &mut Higanbana, _, cx| {
                let c = ui(v);
                c.filters = Default::default();
                c.search.set_text("");
                c.year_from.set_text("");
                c.year_to.set_text("");
                restart(v, cx);
            });
            empty_state(&t, "search", "Nenhuma obra com esses filtros", detail).child(div().pt(8.0).child(clear.render(cx)))
        }
        _ => {
            let query = ui(v).query();
            let detail = (indexed < total).then(|| "Ainda indexando o acervo; mais obras podem aparecer.".into());
            empty_state(&t, "search", format!("Nenhuma obra com “{query}”"), detail)
        }
    }
}

pub fn row(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, r: &Row, ctx: &Ctx) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let (m, width) = (ctx.m, ctx.width);
    let media = v.pages.catalog_media();
    let body = match r {
        Row::Header => header(v, cx, ctx),
        Row::Notice => notice(v, cx, ctx),
        Row::LetterHead { letter, count } => letter_head(&t, *letter, *count),
        Row::Cards { works, start, end } => {
            let prefix = ui(v).series.id_prefix();
            let mut grid = div().gap(GAP_X).pt(ROW_GAP).items_start();
            for e in &works[*start..*end] {
                grid = grid.child(title_card(v, cx, prefix, &e.name, &e.path, media, m.card_w));
            }
            grid
        }
        Row::Loading { letter } => {
            let letter = LETTERS[*letter];
            let mut grid = div().relative().gap(GAP_X).pt(ROW_GAP);
            for i in 0..m.cols {
                grid = grid.child(Skeleton::new(("catalogo-esqueleto", i)).w(m.card_w).h(m.card_w * 1.5).render(cx).rounded(t.radius_lg));
            }
            grid.child(on_visible(400.0, move |v: &mut Higanbana, cx| {
                v.state.want_catalog_letter(media, letter);
                cx.notify();
            }))
        }
        Row::Failed { letter, error } => {
            let name = LETTERS[*letter];
            let text_col = div()
                .flex_col()
                .gap(4.0)
                .child(text(format!("Não foi possível carregar {}", header_label(name))).text_size(t.typography.sm).font_weight(500))
                .child(div().text_size(t.typography.sm).text_color(t.muted_foreground).child(text(format!("{error}. Vamos tentar de novo em instantes."))));
            let card = div().relative().items_start().gap(12.0).p(16.0).rounded(t.radius_lg).border(1.0, t.border).child(icon("triangle-alert").size(16.0).text_color(t.warning)).child(text_col).child(on_visible(400.0, move |v: &mut Higanbana, cx| {
                v.state.want_catalog_letter(media, name);
                cx.notify();
            }));
            div().pt(ROW_GAP).child(card)
        }
        Row::NoneInLetter => div().pt(16.0).text_size(t.typography.sm).text_color(t.muted_foreground).child(text("Nenhuma obra nesta letra.")),
        Row::Footer { next } => {
            let mut d = div().relative().h(super::rows::FOOTER);
            if let Some(next) = *next {
                d = d.child(on_visible(600.0, move |v: &mut Higanbana, cx| {
                    let c = ui(v);
                    if next > c.upto {
                        c.upto = next;
                        let media = c.series.media;
                        v.state.want_catalog_letter(media, LETTERS[next]);
                        cx.notify();
                    }
                }));
            }
            d
        }
    };
    centered(width, body)
}

/// Espaço entre colunas da grade (`gap_x_5`).
pub const GAP_X: f32 = 20.0;
