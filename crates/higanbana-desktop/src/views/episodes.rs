//! Aba Episódios de um anime: cada vídeo vira um card (grade) ou uma linha
//! (lista) com miniatura, título e nota do TMDB quando o arquivo casa com um
//! episódio; sem casamento, miniatura neutra e o rótulo de hoje. Subpastas e
//! arquivos que não são vídeo continuam na lista compacta, abaixo.

use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, button::ButtonVariants as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, assets::IconName, div,
    prelude::*, px, relative, rems,
};
use higanbana_core::{
    anilist::{text, types::FuzzyDate},
    library::join_path,
    nuvem::FileEntry,
    player::logic::{episode_number, video_files},
    settings::EpisodeLayout,
    tmdb::{EpisodeView, WorkEpisodes, still_url},
};

use crate::{
    components::{FileAction, download_button, file_list, rounded_image, when_visible},
    interact::{Button, hover_fade, hover_layer},
    state::{Img, state},
    theme,
};

/// Tamanho do still pedido ao TMDB (grade e lista).
const STILL_SIZE: &str = "w300";
/// Grupo de hover do card: o ▶ aparece ao passar o mouse em qualquer parte dele.
const CARD_GROUP: &str = "ep-card";

/// Rótulo ("E1") e título do card. Sem episódio do TMDB: "Episódio N" quando o
/// arquivo tem número; senão o nome do arquivo, sem rótulo. O número do rótulo
/// é o do arquivo (o que a pessoa vê no player); o do TMDB só quando o arquivo
/// não tem número legível.
pub fn card_text(file: &FileEntry, ep: Option<&EpisodeView>) -> (String, String) {
    let number = episode_number(&file.name).or(ep.map(|e| e.number));
    let label = number.map(|n| format!("E{n}")).unwrap_or_default();
    let title = match (ep, number) {
        (Some(e), _) => e.title.clone(),
        (None, Some(n)) => format!("Episódio {n}"),
        (None, None) => file.name.clone(),
    };
    (label, title)
}

/// Data de exibição do TMDB (`2023-09-29`) no formato das datas do AniList ("29 set 2023").
fn air_date(date: Option<&str>) -> Option<String> {
    let mut parts = date?.split('-').map(|p| p.parse::<u32>().ok());
    let year = parts.next().flatten()? as i32;
    let month = parts.next().flatten();
    let day = parts.next().flatten();
    text::date(Some(FuzzyDate { year: Some(year), month, day }))
}

/// O que um card precisa saber de um vídeo, já lido do estado.
struct Item {
    file: FileEntry,
    path: String,
    key: SharedString,
    label: String,
    title: String,
    still: Option<String>,
    score: Option<String>,
    date: Option<String>,
    overview: String,
    minutes: Option<u32>,
    /// Fração assistida (0–1); `None` quando nunca abriu.
    seen: Option<f32>,
    watched: bool,
}

/// Lista de episódios de uma pasta de anime.
///
/// `eps`: `None` enquanto o TMDB responde (esqueleto nas miniaturas);
/// `Some(None)` sem dados do TMDB (miniatura neutra). `cols` é o número de
/// colunas da grade, medido por quem chama.
pub fn episode_list(
    dir: &str,
    files: &[FileEntry],
    eps: Option<Option<&WorkEpisodes>>,
    layout: EpisodeLayout,
    cols: usize,
    on_play: FileAction,
    cx: &mut App,
) -> Div {
    let videos = video_files(files);
    if videos.is_empty() {
        return file_list(dir, files, Some(on_play), None, cx);
    }
    let others: Vec<FileEntry> = files.iter().filter(|f| !videos.iter().any(|v| v.name == f.name)).cloned().collect();
    let loading = eps.is_none();
    let tmdb = eps.flatten();

    let s = state(cx);
    let items: Vec<Item> = {
        let st = s.read(cx);
        let ratings = st.rating_source();
        videos
            .into_iter()
            .map(|f| {
                let path = join_path(dir, &f.name);
                let ep = tmdb.and_then(|w| w.link(&f.name));
                let (label, title) = card_text(&f, ep);
                let p = st.player_data.progress.get(&path);
                let minutes = ep.and_then(|e| e.runtime).filter(|&m| m > 0).or_else(|| {
                    p.filter(|p| p.duration > 0.).map(|p| (p.duration / 60.).round() as u32)
                });
                Item {
                    key: SharedString::from(f.name.clone()),
                    label,
                    title,
                    still: ep.and_then(|e| e.still_path.as_deref()).map(|p| still_url(p, STILL_SIZE)),
                    score: ep.and_then(|e| ratings.episode(e)).map(|s| s.label()),
                    date: ep.and_then(|e| air_date(e.air_date.as_deref())),
                    overview: ep.map(|e| e.overview.clone()).unwrap_or_default(),
                    minutes,
                    seen: p.map(|p| if p.watched { 1. } else { (p.position / p.duration.max(1.)).clamp(0., 1.) as f32 }),
                    watched: p.is_some_and(|p| p.watched),
                    file: f,
                    path,
                }
            })
            .collect()
    };

    let toggle = {
        let (icon, tip, next) = match layout {
            EpisodeLayout::Grid => (IconName::LayoutList, "Mostrar em lista", EpisodeLayout::List),
            EpisodeLayout::List => (IconName::LayoutGrid, "Mostrar em grade", EpisodeLayout::Grid),
        };
        let s = s.clone();
        Button::new("episode-layout")
            .ghost()
            .small()
            .icon(icon)
            .tooltip(tip)
            .on_click(move |_, _, cx| s.update(cx, |s, cx| s.set_episode_layout(next, cx)))
    };

    let body = match layout {
        EpisodeLayout::Grid => div()
            .grid()
            .grid_cols(cols.max(1) as u16)
            .gap_x_4()
            .gap_y_6()
            .children(items.into_iter().map(|it| grid_card(it, loading, &on_play, cx)))
            .into_any_element(),
        EpisodeLayout::List => {
            let border = cx.theme().border;
            v_flex()
                .children(items.into_iter().enumerate().map(|(i, it)| {
                    div().when(i > 0, |d| d.border_t_1().border_color(border)).child(list_row(it, loading, &on_play, cx))
                }))
                .into_any_element()
        }
    };

    v_flex()
        .gap_3()
        .child(h_flex().justify_end().child(toggle))
        .child(body)
        .when(!others.is_empty(), |d| d.child(div().pt_3().child(file_list(dir, &others, None, None, cx))))
}

/// Card da grade: miniatura 16:9 e, embaixo, rótulo, título, nota e download.
fn grid_card(it: Item, loading: bool, on_play: &FileAction, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let (act, f) = (on_play.clone(), it.file.clone());
    let dl = download_button(ElementId::Name(format!("ep-dl:{}", it.key).into()), &it.path, cx);
    let thumb = thumbnail(&it, loading, cx);
    v_flex()
        .id(ElementId::Name(format!("ep-card:{}", it.key).into()))
        .group(CARD_GROUP)
        .min_w_0()
        .gap_2()
        .cursor_pointer()
        .on_click(move |_, w, cx| act(&f, w, cx))
        .child(thumb)
        .child(
            h_flex()
                .items_start()
                .gap_2()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .when(!it.label.is_empty(), |d| d.child(div().text_xs().text_color(theme.muted_foreground).child(it.label.clone())))
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).line_clamp(2).child(it.title.clone()))
                        .children(it.score.clone().map(|s| score(s, cx))),
                )
                .child(div().flex_none().child(dl)),
        )
        .into_any_element()
}

/// Linha da lista: miniatura à esquerda; rótulo · data, título, nota e sinopse; download à direita.
fn list_row(it: Item, loading: bool, on_play: &FileAction, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let (act, f) = (on_play.clone(), it.file.clone());
    let dl = download_button(ElementId::Name(format!("ep-dl:{}", it.key).into()), &it.path, cx);
    let thumb = thumbnail(&it, loading, cx);
    let meta = [Some(it.label.clone()).filter(|l| !l.is_empty()), it.date.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let row = h_flex()
        .id(ElementId::Name(format!("ep-row:{}", it.key).into()))
        .group(CARD_GROUP)
        .items_start()
        .gap_4()
        .p_2()
        .cursor_pointer()
        .on_click(move |_, w, cx| act(&f, w, cx))
        .child(div().w(rems(10.5)).flex_none().child(thumb))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .when(!meta.is_empty(), |d| d.child(div().text_xs().text_color(theme.muted_foreground).child(meta)))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).line_clamp(2).child(it.title.clone()))
                .children(it.score.clone().map(|s| score(s, cx)))
                .when(!it.overview.is_empty(), |d| {
                    d.child(div().text_xs().text_color(theme.muted_foreground).line_height(relative(1.4)).line_clamp(3).child(it.overview.clone()))
                }),
        )
        .child(div().flex_none().child(dl));
    hover_fade(ElementId::Name(format!("ep-row-hover:{}", it.key).into()), row)
        .fill(theme.list_hover)
        .radius(theme.radius_lg)
        .into_any_element()
}

/// Nota do episódio ("★ 8,5").
fn score(label: String, cx: &App) -> Div {
    h_flex()
        .gap_1()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .child(Icon::new(IconName::Star).xsmall().text_color(cx.theme().yellow))
        .child(label)
}

/// Miniatura 16:9: still do TMDB (carregado só quando aparece na tela), esqueleto
/// enquanto o TMDB responde ou fundo neutro. Por cima: visto, duração, progresso e ▶ no hover.
fn thumbnail(it: &Item, loading: bool, cx: &mut App) -> Div {
    let theme = cx.theme().clone();
    let radius = theme.radius_lg;
    let image: AnyElement = match (&it.still, loading) {
        (_, true) => Skeleton::new().size_full().rounded(radius).into_any_element(),
        (None, false) => neutral(cx).into_any_element(),
        (Some(url), false) => match state(cx).read(cx).cached_image(url) {
            Some(Img::Ready(_)) => rounded_image(Some(url), Some(radius), cx),
            Some(Img::Failed) => neutral(cx).into_any_element(),
            Some(Img::Loading) => div().size_full().into_any_element(),
            None => {
                let url = url.clone();
                div()
                    .size_full()
                    .child(when_visible(300., move |cx: &mut App| {
                        state(cx).update(cx, |s, cx| {
                            s.image(&url, cx);
                        })
                    }))
                    .into_any_element()
            }
        },
    };
    div()
        .relative()
        .w_full()
        .aspect_ratio(16. / 9.)
        .rounded(radius)
        .overflow_hidden()
        .bg(theme.muted)
        .child(image)
        .when(it.watched, |d| {
            d.child(
                h_flex()
                    .absolute()
                    .top_2()
                    .left_2()
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .rounded(theme.radius)
                    .bg(theme::media_scrim(0.7))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::on_media())
                    .child(Icon::new(IconName::Check).xsmall().text_color(theme.success_foreground))
                    .child("Visto"),
            )
        })
        .children(it.minutes.map(|m| {
            div()
                .absolute()
                .bottom_2()
                .right_2()
                .px_1p5()
                .py_0p5()
                .rounded(theme.radius)
                .bg(theme::media_scrim(0.7))
                .text_xs()
                .text_color(theme::on_media())
                .child(format!("{m} min"))
        }))
        .children(it.seen.filter(|&f| f > 0.).map(|f| {
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(3.))
                .bg(theme::media_track(0.3))
                .child(div().h_full().w(relative(f)).bg(theme.primary))
        }))
        // ▶ ao passar o mouse no card (ou na linha) inteiro.
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.)
                .group_hover(CARD_GROUP, |s| s.opacity(1.))
                .child(
                    div()
                        .size_10()
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme::media_scrim(0.6))
                        .child(Icon::new(IconName::Play).text_color(theme::on_media())),
                ),
        )
        .child(hover_layer("thumb-ring").border(theme.ring).radius(radius))
}

/// Miniatura sem still: superfície do tema com o ícone de vídeo.
fn neutral(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui_kit::linear_gradient(
            135.,
            gpui_kit::linear_color_stop(theme.muted, 0.),
            gpui_kit::linear_color_stop(theme.secondary, 1.),
        ))
        .child(Icon::new(IconName::FilePlay).text_color(theme.muted_foreground))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> FileEntry {
        FileEntry { name: name.to_owned(), is_directory: false, size: 0, modified: String::new(), extension: Some(".mkv".into()) }
    }

    fn episode(number: u32, title: &str) -> EpisodeView {
        EpisodeView {
            season: 1,
            number,
            title: title.to_owned(),
            overview: String::new(),
            air_date: None,
            still_path: None,
            vote_average: 0.,
            vote_count: 0,
            runtime: None,
        }
    }

    #[test]
    fn card_text_with_episode() {
        let f = file("[Grupo] Sousou no Frieren - 01 [1080p].mkv");
        assert_eq!(card_text(&f, Some(&episode(1, "O fim da aventura"))), ("E1".into(), "O fim da aventura".into()));
    }

    #[test]
    fn card_text_without_episode_uses_number() {
        let f = file("[Grupo] Sousou no Frieren - 05 [1080p].mkv");
        assert_eq!(card_text(&f, None), ("E5".into(), "Episódio 5".into()));
    }

    #[test]
    fn card_text_without_number_uses_file_name() {
        let f = file("Abertura sem créditos.mkv");
        assert_eq!(card_text(&f, None), (String::new(), "Abertura sem créditos.mkv".into()));
    }

    #[test]
    fn air_date_like_anilist() {
        assert_eq!(air_date(Some("2023-09-29")).as_deref(), Some("29 set 2023"));
        assert_eq!(air_date(Some("")), None);
        assert_eq!(air_date(None), None);
    }
}
