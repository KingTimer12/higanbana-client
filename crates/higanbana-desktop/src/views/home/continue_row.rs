//! Bloco "Continuar" da entrada: o último episódio ou capítulo em destaque e
//! fileiras com as outras obras em andamento. Tudo do progresso local; a nuvem
//! só é listada no clique, para montar a lista de episódios/capítulos vizinhos.

use gpui_kit::{
    AnimationExt as _, AnyElement, App, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled,
    Window, assets::IconName, div, prelude::*, px,
};
use gpui_kit::component::{ActiveTheme as _, Icon, button::ButtonVariants as _, h_flex, progress::Progress, v_flex};
use gpui_kit::{SpringAnimation, SpringConfig};
use higanbana_core::{
    anilist::{MediaType, types::MediaSummary},
    continue_list::{ContinueItem, reading, watching},
    library::{base_name, folder_title, join_path, parent_path, parse_episode, strip_ext},
    player::logic::video_files,
};

use crate::{
    components::{cover, hex_color, is_comic, stop_click, want_lookup, when_visible},
    interact::{Button, hover_layer},
    motion,
    reader::{ReadTarget, progress_map},
    state::{AppState, Tone, state},
    video::PlayTarget,
};

/// Obras por fileira.
const ROW_LIMIT: usize = 12;
/// Alturas em rem: cabeçalho, destaque e cada fileira (rótulo + cartões).
const HEADER_REM: f32 = 2.25;
const GAP_REM: f32 = 1.;
const FEATURED_REM: f32 = 9.;
const ROW_REM: f32 = 7.75;
const CARD_REM: f32 = 5.;

/// Texto de progresso de um item: episódio e quanto falta, ou quanto já leu.
pub fn item_label(file_name: &str, item: &ContinueItem, reading: bool) -> String {
    if reading {
        let name = folder_title(strip_ext(file_name));
        return if item.done { format!("{name} · lido") } else { format!("{name} · {}% lido", (item.fraction * 100.).round()) };
    }
    let ep = parse_episode(file_name);
    let name = ep.number.map_or_else(|| ep.label.clone(), |n| format!("Ep {n}"));
    match item.remaining_secs {
        _ if item.done => format!("{name} · visto"),
        Some(s) if s < 60 => format!("{name} · falta menos de 1 min"),
        Some(s) => format!("{name} · faltam {} min", (s as f64 / 60.).round()),
        None => name,
    }
}

/// Barra de progresso que enche com mola ao aparecer (e ao mudar de valor).
fn filling_progress(id: String, fraction: f32) -> impl IntoElement {
    Progress::new(ElementId::Name(id.clone().into())).with_spring(
        ElementId::Name(format!("{id}:fill").into()),
        SpringAnimation::new(SpringConfig::new(120., 20., 1.)).to(fraction.clamp(0., 1.)).from(0.),
        |p, v| p.value(v * 100.),
    )
}

/// Seção do acervo de um caminho (para a busca de metadados).
fn media_of(path: &str) -> MediaType {
    if path.starts_with("Mangás/") { MediaType::Manga } else { MediaType::Anime }
}

/// O que o bloco mostra; recalculado do progresso salvo a cada mudança do AppState.
#[derive(Clone, Default, PartialEq)]
struct Shape {
    featured: Option<(ContinueItem, bool)>,
    watching: Vec<ContinueItem>,
    reading: Vec<ContinueItem>,
}

fn shape_of(s: &AppState) -> Shape {
    let mut watch = watching(&s.player_data.progress, ROW_LIMIT + 1);
    let mut read = reading(&progress_map(s), ROW_LIMIT + 1);
    let newest_watch = watch.first().map(|i| i.updated_at);
    let newest_read = read.first().map(|i| i.updated_at);
    let featured = match (newest_watch, newest_read) {
        (Some(w), Some(r)) if r > w => Some((read.remove(0), true)),
        (Some(_), _) => Some((watch.remove(0), false)),
        (None, Some(_)) => Some((read.remove(0), true)),
        (None, None) => None,
    };
    watch.truncate(ROW_LIMIT);
    read.truncate(ROW_LIMIT);
    Shape { featured, watching: watch, reading: read }
}

pub struct ContinueRow {
    state: Entity<AppState>,
    shape: Shape,
}

impl ContinueRow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Progresso e capas chegam pelo AppState.
        cx.observe(&state, |v, state, cx| {
            v.shape = shape_of(state.read(cx));
            cx.notify();
        })
        .detach();
        let shape = shape_of(state.read(cx));
        Self { state, shape }
    }

    /// Altura do bloco em rem (0 = nada para continuar).
    pub fn height_rem(&self) -> f32 {
        if self.shape.featured.is_none() {
            return 0.;
        }
        let rows = [!self.shape.watching.is_empty(), !self.shape.reading.is_empty()].iter().filter(|r| **r).count();
        HEADER_REM + GAP_REM + FEATURED_REM + rows as f32 * (GAP_REM + ROW_REM)
    }

    /// Metadados da obra (`None` enquanto a busca não voltou).
    fn media(&self, item: &ContinueItem, cx: &App) -> Option<MediaSummary> {
        self.state.read(cx).lookup(base_name(&item.work_path), media_of(&item.work_path)).flatten()
    }

    fn title(&self, item: &ContinueItem, cx: &App) -> String {
        self.media(item, cx).map_or_else(|| folder_title(base_name(&item.work_path)), |m| m.title())
    }

    /// Retoma: lista a pasta do arquivo (só neste clique) e abre o player ou o leitor.
    fn open(&self, item: &ContinueItem, is_reading: bool, cx: &mut App) {
        let st = self.state.clone();
        let dir = parent_path(&item.file_path);
        if !is_reading && item.done {
            st.update(cx, |s, cx| s.go(dir, cx));
            return;
        }
        let media = self.media(item, cx);
        let title = self.title(item, cx);
        let (file_path, work_path) = (item.file_path.clone(), item.work_path.clone());
        let core = st.read(cx).core.clone();
        let list_dir = dir.clone();
        let task = core.clone().spawn(async move { core.cloud().list(&list_dir).await.map(|l| l.files).map_err(|e| e.to_string()) });
        cx.spawn(async move |cx| {
            let listed = task.await.map_err(|e| e.to_string()).and_then(|r| r);
            st.update(cx, |s, cx| {
                let files = match listed {
                    Ok(f) => f,
                    Err(e) => return s.toast(format!("Não foi possível abrir a pasta: {e}"), Tone::Error, cx),
                };
                let Some(file) = files.iter().find(|f| join_path(&dir, &f.name) == file_path).cloned() else {
                    return s.toast("O arquivo não está mais na nuvem.".to_string(), Tone::Error, cx);
                };
                if is_reading {
                    let files = files.into_iter().filter(is_comic).collect();
                    s.read(ReadTarget { dir, file, files, title_path: work_path, title }, cx);
                } else {
                    s.play(
                        PlayTarget {
                            files: video_files(&files),
                            file,
                            is_anime: dir.starts_with("Animes/"),
                            mal_id: media.and_then(|m| m.id_mal).and_then(|v| u32::try_from(v).ok()),
                            anime_title: title,
                            dir,
                        },
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn featured(&self, item: &ContinueItem, is_reading: bool, rem: f32, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let looked = self.state.read(cx).lookup(base_name(&item.work_path), media_of(&item.work_path));
        let media = looked.clone().flatten();
        let title = self.title(item, cx);
        let label = item_label(base_name(&item.file_path), item, is_reading);
        let (action, icon) = match (is_reading, item.done) {
            (true, _) => ("Continuar lendo", IconName::BookOpen),
            (false, false) => ("Continuar", IconName::Play),
            (false, true) => ("Ver episódios", IconName::ChevronRight),
        };
        let glow = hex_color(media.as_ref().and_then(|m| m.cover_image.as_ref()).and_then(|c| c.color.as_deref())).unwrap_or(theme.primary);
        let open_item = item.clone();
        let this = cx.entity().downgrade();
        h_flex()
            .id(ElementId::Name(format!("continue:{}", item.work_path).into()))
            .relative()
            .h(px(FEATURED_REM * rem))
            .px_4()
            .gap_5()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.group_box)
            .overflow_hidden()
            // Luz na cor da capa vinda da esquerda: o cartão ganha o tom da obra.
            .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
                90.,
                gpui_kit::linear_color_stop(glow.opacity(0.3), 0.),
                gpui_kit::linear_color_stop(glow.opacity(0.), 0.65),
            )))
            .child(div().flex_none().rounded(theme.radius).overflow_hidden().shadow_lg().child(cover(media.as_ref(), &title, Some(px(4.75 * rem)), cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .child(div().text_xs().text_color(theme.muted_foreground).child(if is_reading { "Último capítulo aberto" } else { "Último episódio" }))
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).line_clamp(1).child(title))
                    .child(div().text_sm().text_color(theme.muted_foreground).line_clamp(1).child(label))
                    .child(div().pt_1().max_w(px(24. * rem)).child(filling_progress(format!("continue-progress:{}", item.file_path), item.fraction))),
            )
            .child(stop_click(
                "continue-open",
                Button::new("continue-open-btn").primary().icon(Icon::new(icon)).label(action).on_click(move |_, _, cx| {
                    let _ = this.update(cx, |v, cx| v.open(&open_item, is_reading, cx));
                }),
            ))
            .when(looked.is_none(), |d| d.child(when_visible(200., want_lookup(base_name(&item.work_path).to_owned(), media_of(&item.work_path)))))
            .into_any_element()
    }

    fn row(&self, label: &'static str, items: &[ContinueItem], is_reading: bool, rem: f32, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        v_flex()
            .h(px(ROW_REM * rem))
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.muted_foreground).child(label))
            .child(
                h_flex()
                    .id(ElementId::Name(format!("continue-row:{label}").into()))
                    .gap_3()
                    .pb_2()
                    .overflow_x_scroll()
                    .children(items.iter().enumerate().map(|(i, item)| self.card(item, is_reading, i, rem, cx))),
            )
            .into_any_element()
    }

    fn card(&self, item: &ContinueItem, is_reading: bool, i: usize, rem: f32, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let looked = self.state.read(cx).lookup(base_name(&item.work_path), media_of(&item.work_path));
        let media = looked.clone().flatten();
        let title = self.title(item, cx);
        let label = item_label(base_name(&item.file_path), item, is_reading);
        let open_item = item.clone();
        let this = cx.entity().downgrade();
        h_flex()
            .id(ElementId::Name(format!("continue-card:{}", item.work_path).into()))
            .flex_none()
            .relative()
            .w(px(17. * rem))
            .h(px(CARD_REM * rem))
            .pr_3()
            .gap_3()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.group_box)
            .overflow_hidden()
            .cursor_pointer()
            .child(div().flex_none().h_full().child(cover(media.as_ref(), &title, Some(px(CARD_REM * rem / 1.5)), cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).line_clamp(1).child(title))
                    .child(div().text_xs().text_color(theme.muted_foreground).line_clamp(1).child(label))
                    .child(div().pt_1().child(filling_progress(format!("continue-card-progress:{}", item.file_path), item.fraction))),
            )
            .child(hover_layer("ring").border(theme.ring).radius(theme.radius_lg))
            .on_click(move |_, _, cx| {
                let _ = this.update(cx, |v, cx| v.open(&open_item, is_reading, cx));
            })
            .when(looked.is_none(), |d| d.child(when_visible(200., want_lookup(base_name(&item.work_path).to_owned(), media_of(&item.work_path)))))
            .with_animation(ElementId::Name(format!("continue-in:{}", item.work_path).into()), motion::staggered(i), |e, t| e.opacity(t))
            .into_any_element()
    }
}

impl Render for ContinueRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = f32::from(window.rem_size());
        let shape = self.shape.clone();
        let Some((featured, is_reading)) = shape.featured else {
            return div().into_any_element();
        };
        v_flex()
            .size_full()
            .gap(px(GAP_REM * rem))
            .child(h_flex().h(px(HEADER_REM * rem)).child(super::eyebrow(IconName::Play, "Continuar", cx)))
            .child(self.featured(&featured, is_reading, rem, cx))
            .when(!shape.watching.is_empty(), |d| d.child(self.row("Assistindo", &shape.watching, false, rem, cx)))
            .when(!shape.reading.is_empty(), |d| d.child(self.row("Lendo", &shape.reading, true, rem, cx)))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(done: bool, fraction: f32, remaining_secs: Option<u64>) -> ContinueItem {
        ContinueItem { work_path: "w".into(), file_path: "w/f".into(), updated_at: 0, done, fraction, remaining_secs }
    }

    #[test]
    fn label_unwatched_episode_shows_minutes_left() {
        assert_eq!(item_label("[G] Frieren - 07 [1080p].mkv", &item(false, 0.4, Some(840)), false), "Ep 07 · faltam 14 min");
    }

    #[test]
    fn label_under_a_minute() {
        assert_eq!(item_label("Show - 03.mkv", &item(false, 0.99, Some(20)), false), "Ep 03 · falta menos de 1 min");
    }

    #[test]
    fn label_watched_episode() {
        assert_eq!(item_label("Show - 03.mkv", &item(true, 1., None), false), "Ep 03 · visto");
    }

    #[test]
    fn label_episode_without_number_uses_name() {
        assert_eq!(item_label("Filme.mkv", &item(false, 0.5, Some(3600)), false), "Filme · faltam 60 min");
    }

    #[test]
    fn label_reading_shows_percent() {
        assert_eq!(item_label("Gantz G v01.cbz", &item(false, 0.25, None), true), "Gantz G v01 · 25% lido");
        assert_eq!(item_label("Gantz G v01.cbz", &item(true, 1., None), true), "Gantz G v01 · lido");
    }
}
