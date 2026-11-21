//! Início: saudação, destaques do ano, continuar, seções do acervo e descobrir.

mod continue_card;
mod continue_row;
mod discover;
mod hero;
mod hero_slide;
mod sections;
#[cfg(test)]
mod tests;

use std::time::Instant;

use higanbana_core::anilist::{MediaType, Ranking};
use higanbana_gpui::components::CarouselState;
use higanbana_gpui::prelude::*;

use crate::motion::{transition, BASE};
use crate::root::Higanbana;
use crate::ui::cover::content_width;

pub use continue_row::Shape;

/// Estado de interface do início (some ao sair dele).
pub struct HomeUi {
    pub hero: CarouselState,
    pub watch: CarouselState,
    pub read: CarouselState,
    pub disc: CarouselState,
    pub disc_ranking: Ranking,
    /// O que o "Continuar" mostra e quando foi calculado (recalcular custa: varre o progresso).
    pub shape: Option<(Shape, Instant)>,
}

impl Default for HomeUi {
    fn default() -> Self {
        HomeUi {
            hero: CarouselState::new(0).looping(true).auto_advance(hero::AUTO_ADVANCE),
            watch: CarouselState::new(0),
            read: CarouselState::new(0),
            disc: CarouselState::new(0),
            disc_ranking: Ranking::TopRated,
            shape: None,
        }
    }
}

/// Título de bloco: ícone num selo da cor primária + nome.
pub(super) fn eyebrow(t: &Theme, name: &'static str, title: impl Into<std::borrow::Cow<'static, str>>) -> Element<Higanbana> {
    let badge = div().w(32.0).h(32.0).flex_shrink_0().items_center().justify_center().rounded_full().bg(t.primary.alpha(0.14)).child(icon(name).size(14.0).text_color(t.primary));
    div().items_center().gap(10.0).child(badge).child(text(title).text_size(t.typography.xl).font_weight(600))
}

pub fn page(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let width = content_width(v, cx, 1280.0);
    // Banner primeiro: com o cache frio, as listagens da visita vão para ele.
    v.state.want_home_list(MediaType::Anime, Ranking::Popular, hero::SLIDES);
    if v.state.home.slot(MediaType::Anime, Ranking::Popular).is_some_and(|s| s.done) {
        let r = v.pages.home().disc_ranking;
        v.state.want_home_list(MediaType::Anime, r, discover::PER_TAB);
    }
    let name = v.state.user.as_ref().map(|u| u.name.clone());
    let g = cx.animate_from("inicio-saudacao", 0.0, 1.0, transition(BASE));
    let greeting = div()
        .flex_col()
        .gap(4.0)
        .opacity(g)
        .translate_y(8.0 * (1.0 - g))
        .child(text(name.map_or_else(|| "Bem-vindo de volta".to_owned(), |n| format!("Olá, {n}"))).text_size(t.typography.xxl).font_weight(600))
        .child(div().text_color(t.muted_foreground).child(text("Busque com Ctrl K ou escolha uma seção.")));
    let mut col = div().w(width).flex_col().py(32.0).gap(40.0).child(greeting).child(hero::hero(v, cx));
    if let Some(c) = continue_row::block(v, cx) {
        col = col.child(c);
    }
    let sections = div().flex_col().gap(16.0).child(eyebrow(&t, "folder-open", "Seções")).child(sections::grid(v, cx, width));
    div().w_full().flex_col().items_center().child(col.child(sections).child(discover::block(v, cx)))
}
