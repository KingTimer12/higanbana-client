//! Entrada: parede de capas da temporada e "Entrar com Discord".

use crate::interact::Button;
use gpui_kit::{
    Context, Entity, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window, assets::IconName, div,
    prelude::*, px, relative,
};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, h_flex, v_flex,
    button::ButtonVariants as _,
};
use higanbana_core::anilist::types::MediaSummary;

use crate::{
    components::{hex_color, remote_image},
    state::{AppState, state},
};

pub struct LoginView {
    state: Entity<AppState>,
    covers: Vec<MediaSummary>,
    busy: bool,
    error: Option<SharedString>,
}

impl LoginView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let core = state.read(cx).core.clone();
        let task = core.clone().spawn(async move { core.anilist().season().await });
        cx.spawn(async move |this, cx| {
            let list: Vec<MediaSummary> = task.await.ok().and_then(Result::ok).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            let _ = this.update(cx, |v, cx| {
                v.covers = list.into_iter().filter(|m| m.cover_small().is_some()).take(30).collect();
                cx.notify();
            });
        })
        .detach();
        Self { state, covers: Vec::new(), busy: false, error: None }
    }

    fn login(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
        let core = self.state.read(cx).core.clone();
        let task = core.clone().spawn(async move { core.auth().login().await });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |v, cx| {
                v.busy = false;
                match r {
                    Ok(Err(e)) if e.kind() != "cancelled" => v.error = Some(format!("{e}. Tente entrar de novo.").into()),
                    Err(e) => v.error = Some(e.to_string().into()),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for LoginView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let vw = f32::from(window.viewport_size().width);
        let core = self.state.read(cx).core.clone();
        // Parede de capas medida pela janela.
        let cover_w = px(((vw * 0.62) / 5. - rem).max(5. * rem));
        let wall = h_flex()
            .absolute()
            .top(-cover_w)
            .left(relative(0.38))
            .right(-cover_w * 0.5)
            .items_start()
            .gap_4()
            .opacity(0.6)
            .children((0..5).map(|col| {
                v_flex()
                    .gap_4()
                    .when(col % 2 == 1, |d| d.mt(cover_w * 0.5))
                    .children(self.covers.iter().skip(col).step_by(5).map(|m| {
                        let bg = hex_color(m.color()).unwrap_or(theme.muted);
                        let key = self.state.update(cx, |s, scx| s.cover_key(m, true, scx));
                        div().w(cover_w).h(cover_w * 1.5).flex_none().rounded(theme.radius_lg).overflow_hidden().bg(bg).child(remote_image(key.as_deref(), cx))
                    }))
            }));

        let action = if self.busy {
            let (c1, c2) = (core.clone(), core.clone());
            v_flex()
                .gap_3()
                .p_4()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .bg(theme.group_box)
                .child(h_flex().gap_2().child(Spinner::new().color(theme.primary)).child(div().font_weight(FontWeight::SEMIBOLD).child("Continue no seu navegador")))
                .child(div().text_sm().text_color(theme.muted_foreground).child("Abrimos o Discord no navegador em que você já está logado. Clique em Autorizar e volte para cá."))
                .child(
                    h_flex()
                        .gap_2()
                        .child(Button::new("reopen").small().label("Abrir de novo").on_click(move |_, _, _| {
                            let c = c1.clone();
                            c1.spawn(async move { c.auth().reopen_login().await });
                        }))
                        .child(Button::new("cancel").small().ghost().label("Cancelar").on_click(move |_, _, _| {
                            let c = c2.clone();
                            c2.spawn(async move { c.auth().cancel_login().await });
                        })),
                )
                .into_any_element()
        } else {
            Button::new("discord").primary().large().icon(IconName::LogIn).label("Entrar com Discord").on_click(cx.listener(|v, _, _, cx| v.login(cx))).into_any_element()
        };

        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .child(wall)
            .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                90.,
                gpui_kit::linear_color_stop(theme.background, 0.35),
                gpui_kit::linear_color_stop(theme.background.opacity(0.25), 1.),
            )))
            .child(
                h_flex().relative().size_full().px(px((vw * 0.08).clamp(2. * rem, 7.5 * rem))).child(
                    v_flex()
                        .w_112()
                        .gap_8()
                        .child(
                            v_flex()
                                .gap_3()
                                .child(div().text_3xl().font_weight(FontWeight::BOLD).text_color(theme.primary).child("higanbana"))
                                .child(div().max_w_96().text_lg().text_color(theme.muted_foreground).child("A nuvem de animes da Anitsu, com capa, sinopse e ficha técnica de cada título.")),
                        )
                        .child(v_flex().items_start().gap_3().child(action).children(self.error.clone().map(|e| div().text_sm().text_color(theme.danger_foreground).child(e))))
                        .child(div().text_xs().text_color(theme.muted_foreground).child("Capas e informações do AniList.")),
                ),
            )
    }
}
