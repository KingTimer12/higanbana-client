//! Encarte do álbum ("encarte de CD japonês"): capa com obi, faixas por disco, arquivos.

use crate::interact::hover_layer;
use crate::interact::Button;
use base64::Engine as _;
use gpui_kit::{
    Context, ElementId, Entity, EventEmitter, FontWeight, IntoElement, ObjectFit, ParentElement, Render, Styled,
    Subscription, Window, assets::IconName, div, img, prelude::*, px, relative,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex,
    button::ButtonVariants as _,
    tab::{Tab, TabBar},
};
use higanbana_core::{
    music::album::{Album, format_duration, format_label},
    nuvem::FileEntry,
};

pub use higanbana_core::music::album::looks_like_album;

use crate::{
    components::file_list,
    state::{AppState, Img, state},
    theme::ProductColors as _,
    ui::truncate,
};

pub enum AlbumEvent {
    /// Não era álbum (nada tocável): quem abriu mostra a lista de arquivos.
    NotAlbum,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Tracks,
    Files,
}

pub struct AlbumView {
    state: Entity<AppState>,
    work: Option<String>,
    album: Option<Album>,
    tab: Section,
    cover_key: Option<String>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<AlbumEvent> for AlbumView {}

fn pad(n: Option<u32>, i: usize) -> String {
    format!("{:02}", n.unwrap_or(i as u32 + 1))
}

impl AlbumView {
    pub fn new(path: String, files: Vec<FileEntry>, work: Option<String>, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let subs = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        let core = state.read(cx).core.clone();
        let task = core.clone().spawn(async move { core.cloud().load_album(&path, &files).await });
        cx.spawn(async move |this, cx| {
            let album = task.await.ok().flatten();
            let _ = this.update(cx, |v, cx| match album {
                None => cx.emit(AlbumEvent::NotAlbum),
                Some(a) => {
                    v.state.update(cx, |s, cx| s.music_show(&a, cx));
                    v.load_cover(&a, cx);
                    v.album = Some(a);
                    cx.notify();
                }
            });
        })
        .detach();
        Self { state, work, album: None, tab: Section::Tracks, cover_key: None, _subs: subs }
    }

    /// Capa da pasta; sem ela, a capa embutida na primeira faixa.
    fn load_cover(&mut self, a: &Album, cx: &mut Context<Self>) {
        let key = format!("album-cover:{}", a.path);
        self.cover_key = Some(key.clone());
        if self.state.read(cx).cached_image(&key).is_some() {
            return;
        }
        self.state.update(cx, |s, _| s.mark_image_loading(&key));
        let core = self.state.read(cx).core.clone();
        let cover = a.cover_path.clone().zip(a.cover_size);
        let first = a.tracks.first().cloned();
        let task = core.clone().spawn(async move {
            if let Some((path, size)) = cover
                && let Ok(b) = core.cloud().image(&path, size).await
            {
                return Some(b);
            }
            let t = first?;
            let meta = core.music().meta(&t.path, t.size, &t.ext, true).await.ok()?;
            let data = meta.cover?;
            base64::engine::general_purpose::STANDARD.decode(data.split_once(',')?.1).ok()
        });
        let state = self.state.clone();
        cx.spawn(async move |_, cx| {
            let bytes = task.await.ok().flatten();
            state.update(cx, |s, cx| match bytes {
                Some(b) => s.put_image(key, b, cx),
                None => s.fail_image(&key, cx),
            });
        })
        .detach();
    }
}

impl Render for AlbumView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(a) = self.album.clone() else { return Skeleton::new().h_96().rounded(theme.radius_lg).into_any_element() };
        let rem = f32::from(window.rem_size());
        let st = self.state.read(cx);
        let current = st.is_current_album(&a);
        let live = if current { st.music.clone() } else { None };
        let metas = st.music_session.metas.clone();
        let progress = st.core.music().store.data().progress.get(&a.path).copied();
        let cover = self.cover_key.as_ref().and_then(|k| st.cached_image(k));

        let obi = a.catalog.clone().or(a.year.clone());
        let format = live.as_ref().and_then(|l| l.format.clone()).map(|f| format_label(&f.codec, f.bits, f.sample_rate, f.lossless, f.resampled_to)).or_else(|| {
            a.tracks.iter().find_map(|t| metas.get(&t.path)).map(|m| format_label(&m.codec, m.bits, m.sample_rate, m.lossless, None))
        });
        let durations: Vec<Option<f64>> = a.tracks.iter().map(|t| metas.get(&t.path).and_then(|m| m.duration)).collect();
        let total_secs = durations.iter().all(Option::is_some).then(|| durations.iter().flatten().sum::<f64>());
        let resume = progress.filter(|p| (p.index > 0 || p.position > 5.) && p.index < a.tracks.len());
        let playing = live.as_ref().is_some_and(|l| l.playing);
        let ended = live.as_ref().is_some_and(|l| l.ended);
        let current_index = live.as_ref().and_then(|l| l.index);

        let cover_el = match cover {
            Some(Img::Ready(image)) => img(image).size_full().object_fit(ObjectFit::Cover).into_any_element(),
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.muted_foreground)
                .child(a.title.clone())
                .into_any_element(),
        };

        // --- Ações: a principal toca/pausa; "continuar" só quando outro álbum está tocando ---
        let st_entity = self.state.clone();
        let (main_label, main_icon) = if current && playing {
            ("Pausar", IconName::Pause)
        } else if current && !ended {
            ("Continuar", IconName::Play)
        } else {
            ("Tocar", IconName::Play)
        };
        let album_for_play = a.clone();
        let main_button = Button::new("album-play").primary().icon(main_icon).label(main_label).on_click({
            let st = st_entity.clone();
            move |_, _, cx| {
                st.update(cx, |s, cx| {
                    if current && playing {
                        s.music_command("pause");
                    } else if current && !ended {
                        s.music_command("resume");
                    } else {
                        s.music_open(album_for_play.clone(), 0, 0., cx);
                    }
                })
            }
        });
        let resume_button = resume.filter(|_| !current).map(|p| {
            let st = st_entity.clone();
            let al = a.clone();
            Button::new("album-resume")
                .outline()
                .label(format!("Continuar faixa {} ({})", p.index + 1, format_duration(p.position)))
                .on_click(move |_, _, cx| st.update(cx, |s, cx| s.music_open(al.clone(), p.index, p.position, cx)))
        });

        let art = px(14. * rem);
        let aside = v_flex()
            .w(art + px(3.5 * rem))
            .flex_none()
            .gap_4()
            .p_4()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.group_box)
            .child(
                // Capa com o obi (a faixa vertical dos CDs japoneses).
                div()
                    .relative()
                    .w(art + px(1.25 * rem))
                    .child(div().w(art).h(art).overflow_hidden().rounded(theme.radius).bg(theme.muted).shadow_lg().child(cover_el))
                    .children(obi.map(|o| {
                        v_flex()
                            .absolute()
                            .top_0()
                            .right_0()
                            .h_full()
                            .w_8()
                            .items_center()
                            .justify_center()
                            .rounded_r(theme.radius)
                            .bg(theme.primary)
                            .text_color(theme.primary_foreground)
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .children(o.chars().map(|c| div().child(c.to_string())))
                    })),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_xl().font_weight(FontWeight::BOLD).line_height(relative(1.3)).child(a.title.clone()))
                    .child(div().text_sm().text_color(theme.muted_foreground).child([self.work.clone(), a.year.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · "))),
            )
            .child(h_flex().flex_wrap().gap_2().child(main_button).children(resume_button))
            .child(
                v_flex()
                    .gap_0p5()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .children(format)
                    .child(format!("{} faixas{}", a.tracks.len(), total_secs.map(|s| format!(" · {}", format_duration(s))).unwrap_or_default())),
            );

        let body = match self.tab {
            Section::Files => v_flex()
                .gap_3()
                .child(div().text_sm().text_color(theme.muted_foreground).child("Arquivos desta pasta que não são faixas."))
                .child(file_list(&a.path, &a.others, None, None, cx))
                .into_any_element(),
            Section::Tracks => v_flex()
                .gap_4()
                .children(a.discs.iter().map(|&d| {
                    let rows: Vec<(usize, higanbana_core::music::album::Track)> = a.tracks.iter().cloned().enumerate().filter(|(_, t)| t.disc == d).collect();
                    v_flex()
                        .when(a.discs.len() > 1, |x| {
                            x.child(h_flex().mb_1().px_3().gap_2().text_xs().text_color(theme.muted_foreground).child(Icon::new(IconName::Disc3).xsmall()).child(format!("Disco {d}")))
                        })
                        .children(rows.into_iter().map(|(i, t)| {
                            let meta = metas.get(&t.path);
                            let on = current_index == Some(i) && current;
                            let st = st_entity.clone();
                            let al = a.clone();
                            h_flex()
                                .id(ElementId::Name(format!("track:{}", t.path).into()))
                                .gap_3()
                                .px_3()
                                .py_2()
                                .rounded(theme.radius)
                                .relative()
                                .cursor_pointer()
                                .map(|x| if on { x.bg(theme.list_active) } else { x.child(hover_layer("hover").fill(theme.list_hover).radius(theme.radius)) })
                                .child(
                                    div()
                                        .w_10()
                                        .flex_none()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if on { theme.primary_ink() } else { theme.muted_foreground })
                                        .child(if on && playing {
                                            crate::motion::playing_bars(theme.primary_ink()).into_any_element()
                                        } else {
                                            div().child(pad(t.number, i)).into_any_element()
                                        }),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(truncate(meta.and_then(|m| m.title.clone()).filter(|s| !s.is_empty()).unwrap_or(t.title.clone())).when(on, |x| x.text_color(theme.primary_ink())))
                                        .children(meta.and_then(|m| m.artist.clone()).map(|ar| truncate(ar).text_xs().text_color(theme.muted_foreground))),
                                )
                                .child(div().w_12().text_right().text_sm().text_color(theme.muted_foreground).child(meta.and_then(|m| m.duration).map(format_duration).unwrap_or_default()))
                                .on_click(move |_, _, cx| st.update(cx, |s, cx| if on { s.music_command("toggle") } else { s.music_open(al.clone(), i, 0., cx) }))
                        }))
                }))
                .into_any_element(),
        };

        h_flex()
            .w_full()
            .items_start()
            .gap_8()
            .child(aside)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_4()
                    .child(
                        TabBar::new("album-tabs")
                            .underline()
                            .selected_index(if self.tab == Section::Tracks { 0 } else { 1 })
                            .on_click(cx.listener(|v, ix: &usize, _, cx| {
                                v.tab = if *ix == 0 { Section::Tracks } else { Section::Files };
                                cx.notify();
                            }))
                            .child(Tab::new().cursor_pointer().label("Faixas"))
                            .child(Tab::new().cursor_pointer().label(format!("Arquivos {}", a.others.len()))),
                    )
                    .child(body),
            )
            .into_any_element()
    }
}
