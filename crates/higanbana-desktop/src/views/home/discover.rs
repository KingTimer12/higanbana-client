//! "Descobrir": abas de ranking (pílula) e uma fileira de pôsteres que sobem no hover.

use higanbana_core::anilist::{MediaType, Ranking};
use higanbana_gpui::components::{Carousel, Component, Skeleton, TabBar};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;

use super::eyebrow;
use crate::motion::{ease_out_cubic, rise, RISE_DISTANCE};
use crate::root::Higanbana;
use crate::ui::cover::{poster, PosterProps};
use crate::ui::empty_state;

/// Obras por aba.
pub const PER_TAB: usize = 12;
pub const TABS: [(Ranking, &str); 3] = [(Ranking::TopRated, "Mais bem avaliados"), (Ranking::Popular, "Mais populares"), (Ranking::Trending, "Em alta")];
const POSTER: f32 = 144.0;
/// Capa 2:3 + título em duas linhas + formato + folga para subir no hover.
const ROW: f32 = POSTER * 1.5 + 96.0;
const ARROWS: f32 = 48.0;
const GAP: f32 = 16.0;

/// Troca de aba: a lista que não terminou é cancelada e o carrossel volta ao começo.
pub fn pick(v: &mut Higanbana, ranking: Ranking) {
    let ui = v.pages.home();
    if ui.disc_ranking == ranking {
        return;
    }
    let old = std::mem::replace(&mut ui.disc_ranking, ranking);
    ui.disc = higanbana_gpui::components::CarouselState::new(0);
    v.state.home.leave(MediaType::Anime, old);
}

pub fn block(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let now = cx.now();
    let ranking = v.pages.home().disc_ranking;
    let (items, done) = v.state.home.slot(MediaType::Anime, ranking).map_or((Vec::new(), false), |s| (s.items.clone(), s.done));
    let active = TABS.iter().position(|(r, _)| *r == ranking).unwrap_or(0);
    let mut tabs = TabBar::new("descobrir-abas", active, |v: &mut Higanbana, i, _| {
        if let Some((r, _)) = TABS.get(i) {
            pick(v, *r);
        }
    })
    .pill();
    for (_, label) in TABS {
        tabs = tabs.tab(label);
    }
    let tabs = tabs.render(cx);
    let row = if items.is_empty() && done {
        div().h(ROW).items_center().justify_center().child(empty_state(&t, "inbox", "Nada desta lista no acervo", Some("Tente outra aba ou o outro tipo.".into())))
    } else if items.is_empty() {
        let mut r = div().h(ROW).items_start().px(ARROWS).gap(GAP).overflow_hidden();
        for i in 0..8 {
            r = r.child(Skeleton::new(("descobrir-esqueleto", i)).w(POSTER).h(POSTER * 1.5).render(cx).rounded(t.radius_lg));
        }
        r
    } else {
        let mut cards = Vec::with_capacity(items.len());
        for (i, (m, path)) in items.iter().enumerate() {
            let id = ElementId::from(("descobrir", m.summary.id as usize ^ (active << 40)));
            let lift = ease_out_cubic(crate::ui::hover::amount(cx, id));
            let enter = rise(cx, ("descobrir-entra", m.summary.id as usize ^ (active << 40)), i);
            let props = PosterProps { id, name: m.summary.title(), media: Some(std::sync::Arc::new(m.summary.clone())), loading: false, width: POSTER, badge: false, dimmed: false, meta: None };
            let target = path.clone();
            let card = poster(v, cx, props).on_click(move |v: &mut Higanbana, _, cx| {
                v.state.go(target.clone());
                cx.notify();
            });
            cards.push(div().pt(8.0).opacity(enter).translate_y(RISE_DISTANCE * (1.0 - enter) - 6.0 * lift).child(card));
        }
        let ui = v.pages.home();
        if ui.disc.len() != cards.len() {
            ui.disc.set_len(cards.len(), now);
        }
        let state = ui.disc.clone();
        let car = Carousel::new("descobrir-carrossel", &state, |v: &mut Higanbana| &mut v.pages.home().disc).items(cards).item_width(POSTER).gap(GAP).controls(true).render(cx);
        div().h(ROW).px(ARROWS).child(car)
    };
    div().w_full().flex_col().gap(16.0).child(div().h(40.0).items_center().child(eyebrow(&t, "compass", "Descobrir"))).child(div().h(40.0).items_center().child(tabs)).child(row)
}
