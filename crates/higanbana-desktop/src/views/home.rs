//! Início: saudação, seções do acervo e a temporada atual do AniList.

use crate::interact::hover_layer;
use gpui_kit::{
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Window, assets::IconName, div,
    prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use higanbana_core::anilist::{season_at, types::MediaSummary};

use crate::{
    chrome::loading,
    components::{columns, content_width, poster, section_label, section_meta, section_order, section_title, when_visible, PosterProps},
    state::{AppState, Tone, state},
    ui::empty_state,
};

pub struct HomeView {
    state: Entity<AppState>,
    sections: Option<Result<Vec<String>, String>>,
    /// `None` = carregando.
    season: Option<Vec<MediaSummary>>,
}

impl HomeView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let core = state.read(cx).core.clone();

        let c = core.clone();
        let root = core.spawn(async move { c.cloud().list("").await.map_err(|e| e.to_string()) });
        cx.spawn(async move |this, cx| {
            let r = root.await.map_err(|e| e.to_string()).and_then(|r| r);
            let _ = this.update(cx, |v, cx| {
                v.sections = Some(r.map(|l| {
                    let mut s: Vec<String> = l.files.into_iter().filter(|f| f.is_directory).map(|f| f.name).collect();
                    s.sort_by(|a, b| section_order(a, b));
                    s
                }));
                cx.notify();
            });
        })
        .detach();

        let c = core.clone();
        let season = core.spawn(async move { c.anilist().season().await });
        cx.spawn(async move |this, cx| {
            let list = season.await.ok().and_then(Result::ok).and_then(|v| serde_json::from_value::<Vec<MediaSummary>>(v).ok()).unwrap_or_default();
            let _ = this.update(cx, |v, cx| {
                v.season = Some(list);
                cx.notify();
            });
        })
        .detach();

        Self { state, sections: None, season: None }
    }

    fn open_season(&mut self, m: MediaSummary, cx: &mut Context<Self>) {
        let state = self.state.clone();
        if let Some(Some(path)) = state.read(cx).catalog(m.id) {
            state.update(cx, |s, cx| s.go(path, cx));
            return;
        }
        let core = state.read(cx).core.clone();
        let media = m.clone();
        let task = core.clone().spawn(async move { core.check_catalog(&media).await });
        cx.spawn(async move |_, cx| {
            let path = task.await.ok().flatten();
            state.update(cx, |s, cx| match path {
                Some(p) => s.go(p, cx),
                None => s.toast(format!("\"{}\" ainda não está na nuvem.", m.title()), Tone::Info, cx),
            });
        })
        .detach();
    }
}

fn season_label(code: &str) -> &'static str {
    match code {
        "WINTER" => "inverno",
        "SPRING" => "primavera",
        "SUMMER" => "verão",
        _ => "outono",
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, 80. * rem);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let (season_code, year) = season_at(now);
        let name = self.state.read(cx).user.as_ref().map(|u| u.name.clone());

        // --- Seções ---
        let gap = 0.75 * rem;
        let sec_cols = columns(width, 15. * rem, gap) as u16;
        let sections = match &self.sections {
            None => div().grid().grid_cols(sec_cols).gap_3().children((0..6).map(|_| Skeleton::new().h_20().rounded(theme.radius_lg))),
            Some(Err(_)) => empty_state(IconName::WifiOff, "Não foi possível abrir a nuvem", Some("Verifique sua conexão e reabra o app.".into()), cx),
            Some(Ok(list)) => div().grid().grid_cols(sec_cols).gap_3().children(list.iter().map(|s| {
                let (icon_name, blurb) = section_meta(s);
                let target = s.clone();
                let state = self.state.clone();
                h_flex()
                    .id(ElementId::Name(format!("section:{s}").into()))
                    .gap_4()
                    .h_20()
                    .px_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.group_box)
                    .relative()
                    .cursor_pointer()
                    .child(
                        div()
                            .size_10()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(theme.radius)
                            .bg(theme.muted)
                            .child(Icon::new(icon_name).text_color(theme.foreground)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(section_label(s)))
                            .child(div().text_sm().text_color(theme.muted_foreground).line_clamp(1).child(blurb)),
                    )
                    .child(Icon::new(IconName::ChevronRight).small().text_color(theme.muted_foreground))
                    .child(hover_layer("ring").border(theme.ring).radius(theme.radius_lg))
                    .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(target.clone(), cx)))
            })),
        };

        // --- Temporada ---
        let gap = 1.25 * rem;
        let cols = columns(width, 9.5 * rem, gap);
        let card_w = px(((width - (cols as f32 - 1.) * gap) / cols as f32).floor());
        let season = match &self.season {
            None => div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children((0..cols).map(|_| Skeleton::new().w(card_w).h(card_w * 1.5).rounded(theme.radius_lg))),
            Some(list) if list.is_empty() => empty_state(IconName::CircleAlert, "Não foi possível carregar a temporada agora", Some("O AniList não respondeu. Tente de novo mais tarde.".into()), cx),
            Some(list) => {
                let list = list.clone();
                div().grid().grid_cols(cols as u16).gap_x_5().gap_y_8().children(list.into_iter().map(|m| {
                    let known = self.state.read(cx).catalog(m.id);
                    let id = m.id;
                    let media = m.clone();
                    let card = poster(
                        PosterProps {
                            id: ElementId::Name(format!("season:{id}").into()),
                            name: m.title(),
                            media: Some(m.clone()),
                            loading: false,
                            width: card_w,
                            badge: matches!(known, Some(Some(_))),
                            dimmed: matches!(known, Some(None)),
                        },
                        cx,
                    )
                    .on_click(cx.listener(move |v, _, _, cx| v.open_season(media.clone(), cx)));
                    let st = self.state.clone();
                    div().relative().child(card).child(when_visible(200., move |cx| st.update(cx, |s, cx| s.want_catalog(&m, cx))))
                }))
            }
        };
        let (checked, in_library, total) = self.season.as_ref().map_or((0, 0, 0), |l| {
            let st = self.state.read(cx);
            let known: Vec<_> = l.iter().map(|m| st.catalog(m.id)).collect();
            (known.iter().filter(|k| k.is_some()).count(), known.iter().filter(|k| matches!(k, Some(Some(_)))).count(), l.len())
        });
        let aside = (total > 0).then(|| {
            h_flex()
                .gap_2()
                .text_sm()
                .text_color(theme.muted_foreground)
                .when(checked < total, |d| d.child(loading(cx)))
                .child(format!("{in_library} de {total} no acervo"))
                .into_any_element()
        });

        v_flex()
            .w(px(width))
            .mx_auto()
            .py_8()
            .gap_8()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(match name {
                        Some(n) => format!("Olá, {n}"),
                        None => "Bem-vindo de volta".into(),
                    }))
                    .child(div().text_color(theme.muted_foreground).child("Busque com Ctrl K ou escolha uma seção.")),
            )
            .child(sections)
            .child(v_flex().child(section_title(format!("Temporada de {} {year}", season_label(season_code)), aside)).child(season))
    }
}
