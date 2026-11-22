//! Cards de episódio. Grade ("Higanbana"): miniatura com cantos de pétala que
//! trocam no hover, véu vermelho, pétala com ▶ e traço que se estende. Lista
//! ("cinematográfica"): corte diagonal que se abre e a cor do still vazando.

use std::time::Duration;

use higanbana_gpui::components::{Button, Component, ComponentSize, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};
use higanbana_gpui::{Corners, ElementId};

use super::labels::{head_number, kicker};
use super::shapes::{bubble, cut_triangle, lerp, petal_corners, strip_fractions, PETAL_INSET, PETAL_SIZE};
use super::still::{bleed, still};
use super::Item;
use crate::chrome::tip;
use crate::motion::{ease_out_back, ease_out_cubic, rise, transition, RISE_DISTANCE};
use crate::root::Higanbana;
use crate::theme::{media_track, primary_ink};
use crate::ui::download::download_button;
use crate::ui::files::FileAction;
use crate::ui::hover;

/// Hover dos cards: curto, desacelerando.
const CARD_HOVER: Duration = Duration::from_millis(420);
const GRID_ZOOM: f32 = 1.06;
const LIST_ZOOM: f32 = 1.07;
const LIST_THUMB: f32 = 210.0;
const LIST_MIN_H: f32 = 118.0;

fn hover_t(cx: &mut RenderCx<'_, Higanbana>, id: ElementId) -> f32 {
    let on = cx.is_hovered(id);
    cx.animate(ElementId(id.0 ^ 0x6570_686f), if on { 1.0 } else { 0.0 }, transition(CARD_HOVER))
}

fn play_click(on_play: &FileAction, it: &Item) -> impl Fn(&mut Higanbana, &MouseEvent, &mut EventCx) + 'static {
    let (act, f) = (on_play.clone(), it.file.clone());
    move |v, _, cx| act(v, &f, cx)
}

/// "Assistir" (o caminho pelo teclado) e o download; o clique fica neles.
fn actions(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, it: &Item, on_play: &FileAction, play_opacity: f32) -> Element<Higanbana> {
    let play = Button::new(("ep-play", it.path.as_str()), "").variant(Variant::Ghost).size(ComponentSize::Compact).icon(icon("play").size(14.0)).on_click(play_click(on_play, it)).render(cx);
    let play = tip(v, cx, ("dica-ep-play", it.path.as_str()), play, "Assistir");
    let dl = download_button(v, cx, ("ep-dl", it.path.as_str()), &it.path);
    div().flex_shrink_0().gap(4.0).child(div().opacity(play_opacity).child(play)).child(dl)
}

fn watched(t: &Theme) -> Element<Higanbana> {
    div().gap(4.0).items_center().text_color(t.muted_foreground).child(icon("check").size(12.0).text_color(t.success_foreground)).child(text("Visto"))
}

fn score(t: &Theme, label: String) -> Element<Higanbana> {
    div().gap(4.0).items_center().font_weight(500).child(icon("star").size(12.0).text_color(t.base.yellow)).child(text(label))
}

fn note(t: &Theme, n: String) -> Element<Higanbana> {
    div().flex_col().text_size(t.typography.xs).text_color(t.muted_foreground).line_clamp(1).child(text(n))
}

/// Contorno de 1 px que troca de cor no hover.
fn ring(radii: Corners<f32>, rest: Color, hover: Color) -> Element<Higanbana> {
    let layer = |c: Color| div().absolute().top(0.0).left(0.0).size_full().rounded_corners(radii).border(1.0, c);
    div().absolute().top(0.0).left(0.0).size_full().child(layer(rest)).child(layer(hover))
}

/// Barra de progresso no pé da miniatura, recortada pelos cantos de baixo.
fn progress_strip(t: &Theme, w: f32, h: f32, radii: Corners<f32>, fraction: f32) -> Element<Higanbana> {
    let full = strip_fractions(w, h, radii.bottom_left, radii.bottom_right, 1.0);
    let seen = strip_fractions(w, h, radii.bottom_left, radii.bottom_right, fraction);
    let mut d = div().absolute().top(0.0).left(0.0).size_full();
    if full.len() >= 3 {
        d = d.child(polygon(full, media_track(0.3)).absolute().top(0.0).left(0.0).size_full());
    }
    if seen.len() >= 3 {
        d = d.child(polygon(seen, t.primary).absolute().top(0.0).left(0.0).size_full());
    }
    d
}

/// Pétala com ▶ no canto de baixo à direita: cresce passando um pouco e volta.
fn petal(t: &Theme, h: f32, corner: f32) -> Element<Higanbana> {
    let scale = lerp(0.2, 1., ease_out_back(h));
    let b = bubble(PETAL_SIZE / 2. * scale);
    let size = 16. * scale;
    let (right, bottom) = (PETAL_INSET - b.offset - size / 2., PETAL_INSET + 14. * scale - b.offset - size / 2.);
    let g = LinearGradient::new(GradientDir::Right, vec![(0.0, t.primary_hover), (1.0, t.primary_active)]);
    let radii = Corners { top_left: b.arc, top_right: b.rest, bottom_left: b.rest, bottom_right: if b.offset > 0. { b.arc } else { corner } };
    div()
        .absolute()
        .right(b.offset)
        .bottom(b.offset)
        .w(b.side)
        .h(b.side)
        .rounded_corners(radii)
        .opacity((h * 1.5).min(1.))
        .bg_gradient(g)
        .child(div().absolute().right(right).bottom(bottom).child(icon("play").size(size).text_color(t.primary_foreground)))
}

pub fn grid_card(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, it: &Item, i: usize, loading: bool, on_play: &FileAction, w: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let id = ElementId::from(("ep-card", it.path.as_str()));
    let lin = hover_t(cx, id);
    let e = ease_out_cubic(lin);
    let radii = petal_corners(e);
    let h = w * 9.0 / 16.0;
    let image = still(v, cx, it, loading, radii, lerp(1., GRID_ZOOM, e));
    let mut frame = div().relative().w(w).h(h).rounded_corners(radii).bg(t.muted).child(image);
    if e > 0. {
        frame = frame.shadow(t.primary.fade(0.6 * e), 30.0);
        let veil = LinearGradient::new(GradientDir::Right, vec![(0.4, t.primary.fade(0.0)), (1.0, t.primary.fade(0.33 * e))]);
        frame = frame.child(div().absolute().top(0.0).left(0.0).size_full().rounded_corners(radii).bg_gradient(veil));
    }
    if let Some(f) = it.seen.filter(|&f| f > 0.) {
        frame = frame.child(progress_strip(&t, w, h, radii, f));
    }
    if lin > 0. {
        frame = frame.child(petal(&t, lin, radii.bottom_right));
    }
    frame = frame.child(ring(radii, t.border.fade(1. - e), t.primary.fade(0.55 * e)));
    let mut head = div().flex_1().min_w(0.0).items_start().gap(10.0);
    if let Some(n) = head_number(&it.label) {
        head = head.child(div().flex_shrink_0().min_w(28.0).text_size(t.typography.xl).font_weight(800).text_color(t.primary).child(text(n)));
    }
    let mut titles = div().flex_col().flex_1().min_w(0.0).gap(2.0).child(div().flex_col().text_size(t.typography.sm).font_weight(600).line_clamp(2).child(text(it.title.clone())));
    if let Some(n) = it.note.clone() {
        titles = titles.child(note(&t, n));
    }
    let row = div().items_start().gap(8.0).child(head.child(titles)).child(actions(v, cx, it, on_play, 1. - e));
    let line = div().h(1.0).w(pct(lerp(15.0, 100.0, e))).bg_gradient(LinearGradient::new(GradientDir::Right, vec![(0.0, t.primary), (1.0, t.primary.fade(0.0))]));
    let mut card = div().w(w).flex_col().gap(10.0).cursor_pointer().on_click(play_click(on_play, it)).child(frame).child(row).child(line);
    let mut meta = div().gap(10.0).text_size(t.typography.xs);
    let mut any = false;
    if let Some(s) = it.score.clone() {
        meta = meta.child(score(&t, s));
        any = true;
    }
    if let Some(m) = it.minutes {
        meta = meta.child(div().text_color(t.muted_foreground).child(text(format!("{m} min"))));
        any = true;
    }
    if it.watched {
        meta = meta.child(watched(&t));
        any = true;
    }
    if any {
        card = card.child(meta);
    }
    let k = rise(cx, ("ep-entra-grade", it.path.as_str()), i);
    let card = card.opacity(k).translate_y(RISE_DISTANCE * (1.0 - k));
    let card = hover::track(card, id);
    tip(v, cx, ("dica-ep", it.path.as_str()), card, it.file.name.clone())
}

pub fn list_card(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, it: &Item, i: usize, loading: bool, on_play: &FileAction, w: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let id = ElementId::from(("ep-linha", it.path.as_str()));
    let e = ease_out_cubic(hover_t(cx, id));
    let r = t.radius_lg;
    let thumb_radii = Corners { top_left: r, top_right: 0.0, bottom_right: 0.0, bottom_left: r };
    let image = still(v, cx, it, loading, thumb_radii, lerp(1., LIST_ZOOM, e));
    let mut thumb = div().relative().flex_shrink_0().w(LIST_THUMB).h(LIST_MIN_H).rounded_corners(thumb_radii).overflow_hidden().bg(t.muted).child(div().absolute().top(0.0).left(0.0).size_full().child(image));
    if let Some(f) = it.seen.filter(|&f| f > 0.) {
        thumb = thumb.child(progress_strip(&t, LIST_THUMB, LIST_MIN_H, thumb_radii, f));
    }
    if let Some(points) = cut_triangle(1. - e) {
        thumb = thumb.child(polygon(points.to_vec(), t.popover).absolute().top(0.0).left(0.0).size_full());
    }
    let kick = kicker(&it.label, it.date.as_deref());
    let details = [it.minutes.map(|m| format!("{m} min")), Some(it.details.clone()).filter(|d| !d.is_empty())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let mut body = div().flex_col().flex_1().min_w(0.0).px(16.0).py(12.0).gap(4.0);
    if !kick.is_empty() {
        body = body.child(div().flex_col().text_size(t.typography.xs).font_weight(700).text_color(primary_ink(&t)).line_clamp(1).child(text(kick)));
    }
    body = body.child(div().flex_col().text_size(t.typography.base).font_weight(600).line_clamp(2).child(text(it.title.clone())));
    if let Some(n) = it.note.clone() {
        body = body.child(note(&t, n));
    }
    if !it.overview.is_empty() {
        body = body.child(div().flex_col().text_size(t.typography.xs).text_color(t.muted_foreground).line_clamp(2).child(text(it.overview.clone())));
    }
    let mut foot = div().pt(4.0).gap(10.0).items_center().text_size(t.typography.xs);
    if let Some(s) = it.score.clone() {
        foot = foot.child(score(&t, s));
    }
    if !details.is_empty() {
        foot = foot.child(div().min_w(0.0).overflow_hidden().whitespace_nowrap().text_color(t.muted_foreground).child(text(details)));
    }
    if it.watched {
        foot = foot.child(watched(&t));
    }
    body = body.child(foot.child(div().flex_1()).child(actions(v, cx, it, on_play, 1.0)));
    let mut card = div().relative().w(w).min_h(LIST_MIN_H).rounded(r).overflow_hidden().bg(t.popover).cursor_pointer().on_click(play_click(on_play, it));
    if e > 0.
        && let Some(b) = bleed(v, it, r, e) {
            card = card.child(b);
        }
    card = card.child(thumb).child(body).child(ring(Corners::all(r), t.border.fade(1. - e), t.primary.fade(0.47 * e)));
    let k = rise(cx, ("ep-entra-lista", it.path.as_str()), i);
    let card = card.opacity(k).translate_y(RISE_DISTANCE * (1.0 - k));
    let card = hover::track(card, id);
    tip(v, cx, ("dica-ep-l", it.path.as_str()), card, it.file.name.clone())
}
