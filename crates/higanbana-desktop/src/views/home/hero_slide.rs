//! Um slide do banner: fundo com o banner deslizando devagar, véus do tema,
//! capa e texto que sobem em cascata quando o slide fica ativo.

use higanbana_core::anilist::{text as anitext, types::RankedMedia};
use higanbana_gpui::components::{Button, Component, ComponentSize, Tag, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};

use super::hero::AUTO_ADVANCE;
use crate::motion::{rise, RISE_DISTANCE};
use crate::root::Higanbana;
use crate::ui::cover::cover;
use crate::ui::image::remote_image;
use crate::ui::SHADOW_2XL;

/// Quanto o banner desliza durante um slide (fração da largura).
const DRIFT: f32 = 0.05;

/// "Formato · N episódios · ano · estúdio".
pub fn meta(m: &RankedMedia) -> Vec<String> {
    let s = &m.summary;
    let count = match s.media_type {
        Some(higanbana_core::anilist::MediaType::Manga) => s.chapters.map(|n| format!("{n} capítulos")),
        _ => s.episodes.map(|n| format!("{n} episódios")),
    };
    [s.format.as_deref().map(|f| anitext::format(Some(f))).filter(|f| !f.is_empty()), count, s.year().map(|y| y.to_string()), m.main_studio()].into_iter().flatten().collect()
}

pub fn slide(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, m: &RankedMedia, path: &str, active: bool, height: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let s = &m.summary;
    let title = s.title();
    let id = s.id as usize;
    // Cada linha sobe e aparece um pouco depois da anterior (só no slide ativo; recomeça a cada troca).
    let step = |cx: &mut RenderCx<'_, Higanbana>, el: Element<Higanbana>, i: usize| -> Element<Higanbana> {
        if !active {
            return el;
        }
        let k = rise(cx, ("hero-sobe", id * 8 + i), i);
        el.opacity(k).translate_y(RISE_DISTANCE * (1.0 - k))
    };
    let banner = s.banner_image.clone().or_else(|| s.cover().map(str::to_owned));
    let has_banner = s.banner_image.is_some();
    let drift = if active { cx.animate_from(("hero-desliza", id), 0.0, 1.0, crate::motion::transition(AUTO_ADVANCE).fps(15)) } else { 0.0 };
    let backdrop = div()
        .absolute()
        .top(0.0)
        .bottom(0.0)
        .left(pct(-DRIFT * 100.0 * drift))
        .w(pct(100.0 + DRIFT * 100.0))
        .opacity(if has_banner { 0.65 } else { 0.3 })
        .child(remote_image(v, cx, banner.as_deref(), 0.0));
    // Véus do tema: texto legível sobre qualquer banner.
    let veil_side = div().absolute().top(0.0).left(0.0).size_full().bg_gradient(LinearGradient::new(GradientDir::Right, vec![(0.2, t.background.fade(0.95)), (0.9, t.background.fade(0.0))]));
    let veil_bottom = div().absolute().top(0.0).left(0.0).size_full().bg_gradient(LinearGradient::new(GradientDir::Down, vec![(0.45, t.background.fade(0.0)), (1.0, t.background.fade(0.75))]));
    let cover_el = div().flex_shrink_0().rounded(t.radius_lg).shadow(Color::BLACK.fade(0.25), SHADOW_2XL).child(cover(v, cx, Some(s), &title, Some(176.0)));
    let title_el = div().flex_col().text_size(30.0).font_weight(700).line_clamp(2).child(text(title.clone()));
    let mut meta_el = div().gap(12.0).items_center().text_size(t.typography.sm).text_color(t.muted_foreground);
    if let Some(score) = s.average_score {
        meta_el = meta_el.child(div().gap(4.0).items_center().font_weight(600).text_color(t.foreground).child(icon("star").size(14.0).text_color(t.base.yellow)).child(text(format!("{score}%"))));
    }
    let meta = meta(m);
    if !meta.is_empty() {
        meta_el = meta_el.child(text(meta.join(" · ")));
    }
    let mut chips = div().flex_wrap().gap(6.0);
    for c in s.genres.iter().take(4).map(|g| anitext::genre(g)).chain(m.top_tags(3)) {
        chips = chips.child(Tag::secondary(c).small().render(cx));
    }
    let synopsis = anitext::clean_description(m.description.as_deref()).0.join(" ");
    let target = path.to_owned();
    let open = Button::new(("hero-abrir", id), "Abrir").variant(Variant::Default).size(ComponentSize::Sm).icon(icon("chevron-right").size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
        v.state.go(target.clone());
        cx.notify();
    });
    let open = div().pt(4.0).child(open.render(cx));
    let mut text_col = div().flex_col().flex_1().min_w(0.0).max_w(640.0).gap(12.0);
    text_col = text_col.child(step(cx, title_el, 1)).child(step(cx, meta_el, 2)).child(step(cx, chips, 3));
    if !synopsis.is_empty() {
        let syn = div().flex_col().text_size(t.typography.sm).text_color(t.foreground.fade(0.85)).line_clamp(3).child(text(synopsis));
        text_col = text_col.child(step(cx, syn, 4));
    }
    text_col = text_col.child(step(cx, open, 5));
    let content = div().absolute().top(0.0).left(0.0).size_full().items_center().px(40.0).pb(32.0).gap(32.0).child(step(cx, cover_el, 0)).child(text_col);
    div().relative().w_full().h(height).overflow_hidden().child(backdrop).child(veil_side).child(veil_bottom).child(content)
}
