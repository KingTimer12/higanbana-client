//! Barra do player de músicas, embaixo da janela enquanto um álbum toca.

use crate::interact::hover_layer;
use crate::interact::Button;
use gpui_kit::{
    Context, Entity, FontWeight, IntoElement, ObjectFit, ParentElement, Render, Styled, Subscription, Window,
    assets::IconName, div, img, prelude::*, px,
};
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex,
    button::{ButtonRounded, ButtonVariants as _},
    slider::{Slider, SliderEvent, SliderState},
};
use higanbana_core::{
    library::{base_name, parent_path},
    music::{
        album::{album_title, format_duration, parse_track},
        queue::Repeat,
    },
};

use crate::{
    state::{AppState, Img, state},
    ui::truncate,
};

pub struct MusicBar {
    state: Entity<AppState>,
    /// Posição como fração da faixa (0..1).
    seek: Entity<SliderState>,
    volume: Entity<SliderState>,
    /// Fração escolhida enquanto o usuário arrasta (o motor só pula ao soltar).
    seeking: Option<f32>,
    /// Volume antes de silenciar.
    unmuted: Option<f32>,
    _subs: Vec<Subscription>,
}

impl MusicBar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let seek = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.001));
        let volume = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.01));
        let subs = vec![
            cx.observe_in(&state, window, |v, _, window, cx| v.sync(window, cx)),
            cx.subscribe(&seek, |v, _, e: &SliderEvent, cx| match e {
                SliderEvent::Change(value) => {
                    v.seeking = Some(value.start());
                    cx.notify();
                }
                SliderEvent::Release(value) => {
                    let dur = v.state.read(cx).music.as_ref().and_then(|m| m.duration).unwrap_or(0.);
                    v.state.read(cx).core.music().seek(value.start() as f64 * dur);
                    v.seeking = None;
                }
            }),
            cx.subscribe(&volume, |v, _, e: &SliderEvent, cx| {
                let (SliderEvent::Change(value) | SliderEvent::Release(value)) = e;
                v.state.read(cx).core.music().set(Some(value.start()), None, None, None);
            }),
        ];
        Self { state, seek, volume, seeking: None, unmuted: None, _subs: subs }
    }

    /// Leva posição e volume do motor para os controles (menos o que o usuário está arrastando).
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = self.state.read(cx).music.clone() else { return };
        if self.seeking.is_none() {
            let frac = m.duration.filter(|d| *d > 0.).map_or(0., |d| (m.position / d).clamp(0., 1.)) as f32;
            if (self.seek.read(cx).value().start() - frac).abs() > 0.0005 {
                self.seek.update(cx, |s, cx| s.set_value(frac, window, cx));
            }
        }
        if (self.volume.read(cx).value().start() - m.volume).abs() > 0.005 {
            self.volume.update(cx, |s, cx| s.set_value(m.volume, window, cx));
        }
        cx.notify();
    }

    fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        let Some(volume) = self.state.read(cx).music.as_ref().map(|m| m.volume) else { return };
        let next = if volume <= 0.001 {
            self.unmuted.take().unwrap_or(0.8)
        } else {
            self.unmuted = Some(volume);
            0.
        };
        self.state.read(cx).core.music().set(Some(next), None, None, None);
    }
}

impl Render for MusicBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let st = self.state.read(cx);
        let Some(m) = st.music.clone() else { return div().into_any_element() };
        let Some(album_path) = m.album.clone() else { return div().into_any_element() };
        let core = st.core.clone();
        let meta = m.path.as_ref().and_then(|p| st.music_session.metas.get(p)).cloned();
        let cover = st.cached_image(&format!("album-cover:{album_path}"));
        let file_title = m.path.as_deref().map(|p| parse_track(base_name(p)).title).unwrap_or_default();
        let title = m.title.clone().or(meta.as_ref().and_then(|x| x.title.clone())).filter(|t| !t.is_empty()).unwrap_or(file_title);
        let album_name = album_title(base_name(&album_path));
        let work = base_name(&parent_path(&album_path)).to_owned();
        let subtitle = m.artist.clone().unwrap_or_else(|| if work.is_empty() { album_name.clone() } else { format!("{work} · {album_name}") });
        let dur = m.duration.unwrap_or(0.);
        let shown = self.seeking.map_or(m.position, |f| f as f64 * dur);
        let muted = m.volume <= 0.001;
        let vol_icon = if muted {
            IconName::VolumeX
        } else if m.volume < 0.5 {
            IconName::Volume1
        } else {
            IconName::Volume2
        };
        let (repeat_icon, repeat_tip, repeat_next) = match m.repeat {
            Repeat::Off => (IconName::Repeat, "Não repetir", Repeat::All),
            Repeat::All => (IconName::Repeat, "Repetir álbum", Repeat::One),
            Repeat::One => (IconName::Repeat1, "Repetir faixa", Repeat::Off),
        };
        let cmd = |c: &'static str| {
            let core = core.clone();
            move |_: &gpui_kit::ClickEvent, _: &mut Window, _: &mut gpui_kit::App| {
                let _ = core.music().command(c);
            }
        };
        // As laterais têm a mesma largura para os controles ficarem no centro da janela.
        let side = px(17.5 * rem);

        h_flex()
            .id("music-bar")
            .flex_none()
            .h_20()
            .gap_4()
            .px_4()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.background)
            // Faixa atual: capa, título e obra (abre o álbum).
            .child(
                h_flex()
                    .id("music-open-album")
                    .w(side)
                    .flex_none()
                    .gap_3()
                    .p_1()
                    .rounded(theme.radius)
                    .relative()
                    .cursor_pointer()
                    .child(hover_layer("hover").fill(theme.list_hover).radius(theme.radius))
                    .child(div().size_12().flex_none().rounded(theme.radius).overflow_hidden().bg(theme.muted).children(match cover {
                        Some(Img::Ready(i)) => Some(img(i).size_full().object_fit(ObjectFit::Cover)),
                        _ => None,
                    }))
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(truncate(title).text_sm().font_weight(FontWeight::SEMIBOLD))
                            .child(truncate(subtitle).text_xs().text_color(theme.muted_foreground)),
                    )
                    .on_click({
                        let st = self.state.clone();
                        let path = album_path.clone();
                        move |_, _, cx| st.update(cx, |s, cx| s.go(path.clone(), cx))
                    }),
            )
            // Controles e posição.
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("music-shuffle")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Shuffle)
                                    .selected(m.shuffle)
                                    .tooltip(if m.shuffle { "Aleatório ligado" } else { "Aleatório" })
                                    .on_click({
                                        let core = core.clone();
                                        let s = m.shuffle;
                                        move |_, _, _| core.music().set(None, Some(!s), None, None)
                                    }),
                            )
                            .child(Button::new("music-prev").ghost().small().icon(IconName::SkipBack).tooltip("Faixa anterior").on_click(cmd("prev")))
                            .child(
                                Button::new("music-toggle")
                                    .primary()
                                    .rounded(ButtonRounded::Large)
                                    .icon(if m.playing { IconName::Pause } else { IconName::Play })
                                    .tooltip(if m.playing { "Pausar" } else { "Tocar" })
                                    .on_click(cmd("toggle")),
                            )
                            .child(Button::new("music-next").ghost().small().icon(IconName::SkipForward).tooltip("Próxima faixa").on_click(cmd("next")))
                            .child(
                                Button::new("music-repeat")
                                    .ghost()
                                    .small()
                                    .icon(repeat_icon)
                                    .selected(m.repeat != Repeat::Off)
                                    .tooltip(repeat_tip)
                                    .on_click({
                                        let core = core.clone();
                                        move |_, _, _| core.music().set(None, None, Some(repeat_next), None)
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(px(35. * rem))
                            .gap_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(div().w_10().text_right().child(format_duration(shown)))
                            .child(div().flex_1().child(Slider::new(&self.seek).disabled(dur <= 0.).accessibility_label("Posição na faixa")))
                            .child(div().w_10().child(if dur > 0. { format_duration(dur) } else { "--:--".into() })),
                    ),
            )
            // Volume e volume igualado.
            .child(
                h_flex()
                    .w(side)
                    .flex_none()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("music-normalize")
                            .ghost()
                            .xsmall()
                            .label("Volume igualado")
                            .selected(m.normalize)
                            .tooltip(if m.normalize {
                                "Volume igualado entre álbuns (vale a partir da próxima faixa)"
                            } else {
                                "Volume original dos arquivos (vale a partir da próxima faixa)"
                            })
                            .on_click({
                                let core = core.clone();
                                let n = m.normalize;
                                move |_, _, _| core.music().set(None, None, None, Some(!n))
                            }),
                    )
                    .child(
                        Button::new("music-mute")
                            .ghost()
                            .small()
                            .icon(vol_icon)
                            .tooltip(if muted { "Tirar do mudo" } else { "Mudo" })
                            .on_click(cx.listener(|v, _, _, cx| v.toggle_mute(cx))),
                    )
                    .child(div().w_24().child(Slider::new(&self.volume).accessibility_label("Volume"))),
            )
            .into_any_element()
    }
}
