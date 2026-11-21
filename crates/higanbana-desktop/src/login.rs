//! Entrada: parede de capas da temporada e "Entrar com Discord".

use higanbana_core::anilist::types::MediaSummary;
use higanbana_gpui::components::{Button, Component, ComponentSize, Spinner, Variant};
use higanbana_gpui::prelude::*;
use higanbana_gpui::style::{GradientDir, LinearGradient};

use crate::root::Higanbana;
use crate::state::Img;

#[derive(Default)]
pub struct Login {
    asked_covers: bool,
    pub busy: bool,
    pub error: Option<String>,
}

impl Login {
    /// Capas da temporada (uma vez).
    pub fn want_covers(v: &mut Higanbana) {
        if v.login.asked_covers {
            return;
        }
        v.login.asked_covers = true;
        let core = v.state.core.clone();
        v.state.run(async move { core.anilist().season().await.ok().and_then(|j| serde_json::from_value::<Vec<MediaSummary>>(j).ok()).unwrap_or_default() }, |s, list, _| {
            s.season = list.into_iter().filter(|m| m.cover_small().is_some()).take(30).collect();
        });
    }

    fn start(v: &mut Higanbana) {
        v.login.busy = true;
        v.login.error = None;
        let core = v.state.core.clone();
        v.state.run(async move { core.auth().login().await.err().filter(|e| e.kind() != "cancelled").map(|e| e.to_string()) }, |s, err, _| {
            s.login_result = Some(err.map(|e| format!("{e}. Tente entrar de novo.")));
        });
    }
}

fn hex(c: Option<&str>) -> Option<Color> {
    let h = c?.trim_start_matches('#');
    u32::from_str_radix(h, 16).ok().filter(|_| h.len() == 6).map(rgb)
}

fn wall(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>, cover_w: f32) -> Element<Higanbana> {
    let t = cx.theme();
    let (muted, radius) = (t.muted, t.radius_lg);
    let covers = v.state.season.clone();
    let mut row = div().absolute().top(-cover_w).left(pct(38.0)).right(-cover_w * 0.5).items_start().gap(16.0).opacity(0.6);
    for col in 0..5 {
        let mut column = div().flex_col().gap(16.0);
        if col % 2 == 1 {
            column = column.mt(cover_w * 0.5);
        }
        for m in covers.iter().skip(col).step_by(5) {
            let bg = hex(m.color()).unwrap_or(muted);
            let mut card = div().w(cover_w).h(cover_w * 1.5).flex_shrink_0().rounded(radius).overflow_hidden().bg(bg);
            if let Some(url) = m.cover_small()
                && let Img::Ready(src) = v.state.image(url)
            {
                card = card.child(img(src).size_full().rounded(radius));
            }
            column = column.child(card);
        }
        row = row.child(column);
    }
    row
}

fn action(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    if !v.login.busy {
        return Button::new("discord", "Entrar com Discord")
            .variant(Variant::Default)
            .size(ComponentSize::Lg)
            .icon(icon("log-in").size(16.0))
            .on_click(|v: &mut Higanbana, _, cx| {
                Login::start(v);
                cx.notify();
            })
            .render(cx);
    }
    let (c1, c2) = (v.state.core.clone(), v.state.core.clone());
    let reopen = Button::new("reopen", "Abrir de novo").size(ComponentSize::Compact).on_click(move |_: &mut Higanbana, _, _| {
        let c = c1.clone();
        c1.spawn(async move { c.auth().reopen_login().await });
    });
    let cancel = Button::new("cancel", "Cancelar").variant(Variant::Ghost).size(ComponentSize::Compact).on_click(move |_: &mut Higanbana, _, _| {
        let c = c2.clone();
        c2.spawn(async move { c.auth().cancel_login().await });
    });
    let t = cx.theme();
    let (primary, muted, sm, border, group, radius) = (t.primary, t.muted_foreground, t.typography.sm, t.border, t.group_box, t.radius_lg);
    let spinner = Spinner::new("login-spinner").color(primary).render(cx);
    let (reopen, cancel) = (reopen.render(cx), cancel.render(cx));
    div()
        .flex_col()
        .gap(12.0)
        .p(16.0)
        .rounded(radius)
        .border(1.0, border)
        .bg(group)
        .child(div().items_center().gap(8.0).child(spinner).child(text("Continue no seu navegador").font_weight(600)))
        .child(div().text_size(sm).text_color(muted).child(text("Abrimos o Discord no navegador em que você já está logado. Clique em Autorizar e volte para cá.")))
        .child(div().gap(8.0).child(reopen).child(cancel))
}

pub fn view(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    Login::want_covers(v);
    if let Some(result) = v.state.login_result.take() {
        v.login.busy = false;
        v.login.error = result;
    }
    let vw = cx.viewport().width;
    let rem = 16.0;
    let cover_w = ((vw * 0.62) / 5. - rem).max(5. * rem);
    let wall = wall(v, cx, cover_w);
    let action = action(v, cx);
    let t = cx.theme();
    let (bg, primary, muted, danger, sm, xs) = (t.background, t.primary, t.muted_foreground, t.destructive_foreground, t.typography.sm, t.typography.xs);
    let veil = LinearGradient::new(GradientDir::Right, vec![(0.35, bg), (1.0, bg.alpha(0.25))]);
    let mut actions = div().flex_col().items_start().gap(12.0).child(action);
    if let Some(e) = v.login.error.clone() {
        actions = actions.child(div().text_size(sm).text_color(danger).child(text(e)));
    }
    let copy = div()
        .flex_col()
        .w(448.0)
        .gap(32.0)
        .child(
            div()
                .flex_col()
                .gap(12.0)
                .child(text("higanbana").text_size(30.0).font_weight(700).text_color(primary))
                .child(div().max_w(384.0).text_size(18.0).text_color(muted).child(text("A nuvem de animes da Anitsu, com capa, sinopse e ficha técnica de cada título."))),
        )
        .child(actions)
        .child(div().text_size(xs).text_color(muted).child(text("Capas e informações do AniList.")));
    div()
        .relative()
        .size_full()
        .overflow_hidden()
        .child(wall)
        .child(div().absolute().inset(0.0).bg_gradient(veil))
        .child(div().relative().size_full().items_center().px((vw * 0.08).clamp(2. * rem, 7.5 * rem)).child(copy))
}
