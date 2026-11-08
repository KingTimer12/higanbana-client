//! Aba Episódios de um anime: cada vídeo vira um card (grade) ou uma linha
//! (lista) com miniatura, título e nota do TMDB quando o arquivo casa com um
//! episódio; sem casamento, miniatura neutra e o rótulo de hoje. Subpastas e
//! arquivos que não são vídeo continuam na lista compacta, abaixo.

use std::{sync::Arc, time::Duration};

use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, Size, Sizable as _, Theme, button::ButtonVariants as _, h_flex, v_flex};
use gpui_kit::{
    AnimationExt as _, AnyElement, App, BoxShadow, Bounds, Corners, Div, ElementId, FontWeight, Hsla, IntoElement, ObjectFit,
    ParentElement, PathBuilder, Pixels, SharedString, Stateful, Styled, assets::IconName, canvas, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, relative, rems, size,
};
use higanbana_core::{
    anilist::{text, types::FuzzyDate},
    library::{join_path, parse_episode},
    nuvem::FileEntry,
    player::logic::{episode_number, video_files},
    settings::EpisodeLayout,
    tmdb::{EpisodeView, WorkEpisodes, still_url},
};

use crate::{
    components::{FileAction, download_button, file_list, format_bytes, stop_click, when_visible},
    interact::{Button, hover_anim},
    motion::{QUICK, ease_out_back, ease_out_cubic, enter},
    state::{Img, state},
    theme::{self, ProductColors as _},
};

/// Tamanho do still pedido ao TMDB (grade e lista).
const STILL_SIZE: &str = "w300";
/// Still minúsculo esticado por trás do card da lista: a ampliação o borra.
const BLEED_SIZE: &str = "w92";
/// Hover dos cards (cantos, véu, pétala, traço, zoom, corte): curto, desacelerando.
const CARD_HOVER: Duration = Duration::from_millis(420);
/// Cantos da miniatura da grade, estreito e largo alternados como uma pétala;
/// no hover eles trocam de lugar.
const PETAL_TIGHT: f32 = 4.;
const PETAL_WIDE: f32 = 22.;
/// Pétala do ▶: diâmetro aberto e distância do centro dela ao canto da miniatura
/// (o resto fica para fora, cortado).
const PETAL_SIZE: f32 = 90.;
const PETAL_INSET: f32 = 15.;
/// Zoom da imagem no hover (grade e lista) e do fundo borrado da lista.
const GRID_ZOOM: f32 = 1.06;
const LIST_ZOOM: f32 = 1.07;
const BLEED_ZOOM: f32 = 1.4;
/// Opacidade do fundo borrado da lista no hover.
const BLEED_ALPHA: f32 = 0.35;
/// Corte diagonal da miniatura da lista: a borda direita termina embaixo a 86 %.
const LIST_CUT: f32 = 0.14;
/// Altura da barra de progresso.
const BAR_HEIGHT: f32 = 3.;

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

/// Segunda linha dos vídeos cujo rótulo se repete (`01` e `01v2`, 720p e 1080p),
/// para `(rótulo, nome do arquivo)`: a qualidade do nome quando ela separa o
/// grupo; senão o nome do arquivo. Rótulo único ou vazio → `None`.
pub fn disambiguators(items: &[(String, String)]) -> Vec<Option<String>> {
    let quality: Vec<Option<String>> = items.iter().map(|(_, name)| parse_episode(name).quality).collect();
    items
        .iter()
        .enumerate()
        .map(|(i, (label, name))| {
            if label.is_empty() {
                return None;
            }
            let group: Vec<usize> = (0..items.len()).filter(|&j| items[j].0 == *label).collect();
            if group.len() < 2 {
                return None;
            }
            let distinct = group.iter().all(|&j| quality[j].is_some())
                && group.iter().all(|&j| group.iter().filter(|&&k| quality[k] == quality[j]).count() == 1);
            Some(if distinct { quality[i].clone().unwrap_or_default() } else { name.clone() })
        })
        .collect()
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
    /// Segunda linha quando o rótulo se repete (qualidade ou nome do arquivo).
    note: Option<String>,
    /// Formato · tamanho (`MKV · 1,2 GB`), na lista.
    details: String,
    still: Option<String>,
    /// Still minúsculo para o fundo borrado da lista.
    bleed: Option<String>,
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
/// colunas da grade, medido por quem chama. `on_read` vai para a lista
/// compacta dos outros arquivos (quadrinhos soltos na pasta).
#[allow(clippy::too_many_arguments)]
pub fn episode_list(
    dir: &str,
    files: &[FileEntry],
    eps: Option<Option<&WorkEpisodes>>,
    layout: EpisodeLayout,
    cols: usize,
    on_play: FileAction,
    on_read: Option<FileAction>,
    cx: &mut App,
) -> Div {
    let videos = video_files(files);
    if videos.is_empty() {
        return file_list(dir, files, Some(on_play), on_read, cx);
    }
    let others: Vec<FileEntry> = files.iter().filter(|f| !videos.iter().any(|v| v.name == f.name)).cloned().collect();
    let loading = eps.is_none();
    let tmdb = eps.flatten();

    let s = state(cx);
    // Cada arquivo ligado uma vez, com a numeração decidida para a pasta inteira.
    let names: Vec<&str> = videos.iter().map(|f| f.name.as_str()).collect();
    let links: Vec<Option<&EpisodeView>> = tmdb.map_or_else(|| vec![None; names.len()], |w| w.links(&names));
    let texts: Vec<(String, String)> = videos.iter().zip(&links).map(|(f, ep)| card_text(f, *ep)).collect();
    let notes = disambiguators(&texts.iter().zip(&videos).map(|((l, _), f)| (l.clone(), f.name.clone())).collect::<Vec<_>>());
    let items: Vec<Item> = {
        let st = s.read(cx);
        let ratings = st.rating_source();
        videos
            .into_iter()
            .zip(links.iter().zip(texts.into_iter().zip(notes)))
            .map(|(f, (&ep, ((label, title), note)))| {
                let path = join_path(dir, &f.name);
                let ext = f.extension.clone().unwrap_or_default();
                let p = st.player_data.progress.get(&path);
                let minutes = ep.and_then(|e| e.runtime).filter(|&m| m > 0).or_else(|| {
                    p.filter(|p| p.duration > 0.).map(|p| (p.duration / 60.).round() as u32)
                });
                Item {
                    key: SharedString::from(f.name.clone()),
                    label,
                    title,
                    note,
                    details: [ext.trim_start_matches('.').to_uppercase(), format_bytes(f.size)]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                    still: ep.and_then(|e| e.still_path.as_deref()).map(|p| still_url(p, STILL_SIZE)),
                    bleed: ep.and_then(|e| e.still_path.as_deref()).map(|p| still_url(p, BLEED_SIZE)),
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
            .children(items.into_iter().map(|it| grid_card(it, loading, &on_play)))
            .into_any_element(),
        EpisodeLayout::List => div()
            .grid()
            .grid_cols(list_columns(cols) as u16)
            .gap_3()
            .children(items.into_iter().map(|it| list_card(it, loading, &on_play)))
            .into_any_element(),
    };

    v_flex()
        .gap_3()
        .child(h_flex().justify_end().child(toggle))
        .child(body)
        .when(!others.is_empty(), |d| d.child(div().pt_3().child(file_list(dir, &others, None, on_read, cx))))
}

/// Card da grade ("Higanbana"): miniatura com cantos de pétala e, embaixo,
/// número grande, título, traço vermelho, nota e download. No hover os cantos
/// trocam, um véu vermelho cobre a imagem (que dá zoom), uma pétala com ▶
/// desabrocha no canto e o traço se estende.
fn grid_card(it: Item, loading: bool, on_play: &FileAction) -> AnyElement {
    let on_play = on_play.clone();
    let hover_id = ElementId::Name(format!("ep-card-hover:{}", it.key).into());
    hover_anim(hover_id, CARD_HOVER, move |t, _, cx| {
        let e = ease_out_cubic(t);
        let theme = cx.theme().clone();
        let radii = petal_corners(e);
        let actions = actions(&it, &on_play, cx);
        let image = still(&it, loading, radii, lerp(1., GRID_ZOOM, e), cx);
        let (act, f) = (on_play.clone(), it.file.clone());
        let name = it.key.clone();

        let frame = corners(div().relative().w_full().aspect_ratio(16. / 9.).overflow_hidden().bg(theme.muted), radii)
            .when(e > 0., |d| {
                d.shadow(vec![BoxShadow {
                    color: theme.primary.opacity(e),
                    offset: point(px(0.), px(14.)),
                    blur_radius: px(30.),
                    spread_radius: px(-14.),
                    inset: false,
                }])
            })
            .child(image)
            // Véu vermelho na diagonal.
            .when(e > 0., |d| {
                d.child(corners(div().absolute().top_0().left_0().size_full(), radii).bg(linear_gradient(
                    135.,
                    linear_color_stop(theme.primary.opacity(0.), 0.4),
                    linear_color_stop(theme.primary.opacity(0.33 * e), 1.),
                )))
            })
            .when(t > 0., |d| d.child(petal(t, &theme)))
            .children(it.seen.filter(|&f| f > 0.).map(|f| progress_strip(f, radii, &theme)))
            .child(ring(radii, theme.border.opacity(1. - e), theme.primary.opacity(0.55 * e)));

        let meta: Vec<AnyElement> = [
            it.score.clone().map(|s| score(s, cx).into_any_element()),
            it.minutes.map(|m| div().text_color(theme.muted_foreground).child(format!("{m} min")).into_any_element()),
            it.watched.then(|| watched(&theme).into_any_element()),
        ]
        .into_iter()
        .flatten()
        .collect();

        v_flex()
            .id(ElementId::Name(format!("ep-card:{}", it.key).into()))
            .min_w_0()
            .gap_2p5()
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
            .on_click(move |_, w, cx| act(&f, w, cx))
            .child(frame)
            .child(
                h_flex()
                    .items_start()
                    .gap_2()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_baseline()
                            .gap_2p5()
                            .children(head_number(&it.label).map(|n| {
                                div()
                                    .flex_none()
                                    .min_w(rems(1.75))
                                    .text_xl()
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .line_height(relative(1.))
                                    .text_color(theme.primary)
                                    .child(n)
                            }))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).line_clamp(2).child(it.title.clone()))
                                    .children(it.note.clone().map(|n| note(n, cx))),
                            ),
                    )
                    .child(actions),
            )
            // Traço que sai do número e se estende no hover.
            .child(div().h(px(1.)).w(relative(lerp(0.15, 1., e))).bg(linear_gradient(
                90.,
                linear_color_stop(theme.primary, 0.),
                linear_color_stop(theme.primary.opacity(0.), 1.),
            )))
            .when(!meta.is_empty(), |d| d.child(h_flex().gap_2p5().text_xs().children(meta)))
    })
    .into_any_element()
}

/// Pétala com ▶ no canto de baixo à direita da miniatura, com a ponta virada
/// para o centro da imagem. `t` é o hover linear: cresce passando um pouco do
/// tamanho e volta, enquanto aparece. Só decora: o ▶ "Assistir" do card é o
/// caminho pelo teclado.
fn petal(t: f32, theme: &Theme) -> Div {
    let scale = lerp(0.2, 1., ease_out_back(t));
    let side = PETAL_SIZE * scale;
    let round = px(side / 2.);
    let offset = px(PETAL_INSET - side / 2.);
    let icon = 16. * scale;
    // ▶ no meio da parte que fica à vista (o quarto de cima à esquerda).
    let at = px(side * (35. / 90.) - icon / 2.);
    div()
        .absolute()
        .right(offset)
        .bottom(offset)
        .size(px(side))
        .rounded_tr(round)
        .rounded_br(round)
        .rounded_bl(round)
        .opacity((t * 1.5).min(1.))
        .bg(linear_gradient(135., linear_color_stop(theme.primary_hover, 0.), linear_color_stop(theme.primary_active, 1.)))
        .child(
            div()
                .absolute()
                .left(at)
                .top(at)
                .child(Icon::new(IconName::Play).with_size(Size::Size(px(icon))).text_color(theme.primary_foreground)),
        )
}

/// Contorno de 1 px que troca de cor no hover (as duas cores se cruzam).
fn ring(radii: Corners<Pixels>, rest: Hsla, hover: Hsla) -> Div {
    div().absolute().top_0().left_0().size_full().children([rest, hover].into_iter().filter(|c| c.a > 0.).map(move |c| {
        corners(div().absolute().top_0().left_0().size_full().border_1().border_color(c), radii)
    }))
}

/// "✓ Visto": ícone e texto, não só cor.
fn watched(theme: &Theme) -> Div {
    h_flex()
        .gap_1()
        .text_color(theme.muted_foreground)
        .child(Icon::new(IconName::Check).xsmall().text_color(theme.success_foreground))
        .child("Visto")
}

/// Card da lista ("lista cinematográfica"): miniatura com corte diagonal à
/// esquerda; episódio · data, título, nota, sinopse e detalhes à direita. No
/// hover o corte se abre, a imagem dá zoom e a cor dela vaza, borrada, pelo
/// fundo do card.
fn list_card(it: Item, loading: bool, on_play: &FileAction) -> AnyElement {
    let on_play = on_play.clone();
    let hover_id = ElementId::Name(format!("ep-row-hover:{}", it.key).into());
    hover_anim(hover_id, CARD_HOVER, move |t, _, cx| {
        let e = ease_out_cubic(t);
        let theme = cx.theme().clone();
        let r = theme.radius_lg;
        let card_radii = Corners::all(r);
        let zero = px(0.);
        let thumb_radii = Corners { top_left: r, top_right: zero, bottom_right: zero, bottom_left: r };
        let actions = actions(&it, &on_play, cx);
        let image = still(&it, loading, thumb_radii, lerp(1., LIST_ZOOM, e), cx);
        let bleed = (e > 0.).then(|| bleed(&it, card_radii, e, cx)).flatten();
        let (act, f) = (on_play.clone(), it.file.clone());
        let name = it.key.clone();
        let kicker = kicker(&it.label, it.date.as_deref());
        let details = [it.minutes.map(|m| format!("{m} min")), Some(it.details.clone()).filter(|d| !d.is_empty())]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");

        let thumb = div()
            .relative()
            .flex_none()
            .w(rems(13.125))
            .overflow_hidden()
            .bg(theme.muted)
            .rounded_tl(r)
            .rounded_bl(r)
            // Fora do fluxo: a miniatura estica até a altura do card.
            .child(div().absolute().top_0().left_0().size_full().child(image))
            .children(it.seen.filter(|&f| f > 0.).map(|f| progress_strip(f, thumb_radii, &theme)))
            .child(diagonal_cut(1. - e, theme.popover));

        let body = v_flex()
            .flex_1()
            .min_w_0()
            .px_4()
            .py_3()
            .gap_1()
            .when(!kicker.is_empty(), |d| {
                d.child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.primary_ink()).line_clamp(1).child(kicker))
            })
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).line_clamp(2).child(it.title.clone()))
            .children(it.note.clone().map(|n| note(n, cx)))
            .when(!it.overview.is_empty(), |d| {
                d.child(div().text_xs().text_color(theme.muted_foreground).line_height(relative(1.4)).line_clamp(2).child(it.overview.clone()))
            })
            .child(
                h_flex()
                    .mt_auto()
                    .pt_1()
                    .gap_2p5()
                    .text_xs()
                    .children(it.score.clone().map(|s| score(s, cx)))
                    .when(!details.is_empty(), |d| d.child(div().min_w_0().truncate().text_color(theme.muted_foreground).child(details)))
                    .when(it.watched, |d| d.child(watched(&theme)))
                    .child(div().flex_1())
                    .child(actions),
            );

        div()
            .id(ElementId::Name(format!("ep-row:{}", it.key).into()))
            .relative()
            .flex()
            .min_w_0()
            .min_h(rems(7.375))
            .rounded(r)
            .overflow_hidden()
            .bg(theme.popover)
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
            .on_click(move |_, w, cx| act(&f, w, cx))
            .children(bleed)
            .child(thumb)
            .child(body)
            .child(ring(card_radii, theme.border.opacity(1. - e), theme.primary.opacity(0.47 * e)))
    })
    .into_any_element()
}

/// Fundo borrado do card da lista: o still minúsculo (`w92`) esticado sobre o
/// card inteiro, fraco; a ampliação faz o papel do desfoque. Só é pedido no
/// primeiro hover, porque só aparece nele.
fn bleed(it: &Item, radii: Corners<Pixels>, e: f32, cx: &mut App) -> Option<AnyElement> {
    let url = it.bleed.as_ref()?;
    let image = match state(cx).read(cx).cached_image(url) {
        Some(Img::Ready(image)) => image,
        Some(_) => return None,
        None => {
            state(cx).update(cx, |s, cx| {
                s.image(url, cx);
            });
            return None;
        }
    };
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .opacity(BLEED_ALPHA * e)
            .child(zoomed_image(image, BLEED_ZOOM, radii))
            .into_any_element(),
    )
}

/// Triângulo na cor do fundo do card sobre a borda direita da miniatura: o
/// corte diagonal (de cima à direita até 86 % embaixo). `amount` 1 = corte
/// inteiro, 0 = aberto.
fn diagonal_cut(amount: f32, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let Some(points) = cut_triangle(bounds, amount) else {
                return;
            };
            let mut path = PathBuilder::fill();
            path.add_polygon(&points, true);
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Pontos do corte diagonal em `b` com `amount` do corte (0 = nenhum).
fn cut_triangle(b: Bounds<Pixels>, amount: f32) -> Option<[gpui_kit::Point<Pixels>; 3]> {
    let cut = b.size.width * (LIST_CUT * amount.clamp(0., 1.));
    (cut > px(0.)).then(|| {
        let br = b.bottom_right();
        [b.top_right(), br, point(br.x - cut, br.y)]
    })
}

/// Botões do card/linha: "Assistir" (o caminho pelo teclado até um episódio:
/// o `Button` do kit é parada de Tab e responde a Enter/Espaço) e o download.
/// A área para o clique, para baixar não tocar o episódio.
fn actions(it: &Item, on_play: &FileAction, cx: &mut App) -> Stateful<Div> {
    let (act, f) = (on_play.clone(), it.file.clone());
    let dl = download_button(ElementId::Name(format!("ep-dl:{}", it.key).into()), &it.path, cx);
    stop_click(
        ElementId::Name(format!("ep-actions:{}", it.key).into()),
        h_flex()
            .flex_none()
            .gap_1()
            .child(
                Button::new(ElementId::Name(format!("ep-play:{}", it.key).into()))
                    .ghost()
                    .small()
                    .icon(IconName::Play)
                    .tooltip("Assistir")
                    .on_click(move |_, w, cx| act(&f, w, cx)),
            )
            .child(dl),
    )
}

/// Linha que separa arquivos com o mesmo rótulo (qualidade ou nome do arquivo).
fn note(text: String, cx: &App) -> Div {
    div().text_xs().text_color(cx.theme().muted_foreground).line_clamp(1).child(text)
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Aplica raios por canto (os helpers do GPUI só têm o mesmo raio nos quatro).
fn corners<E: Styled>(e: E, r: Corners<Pixels>) -> E {
    e.rounded_tl(r.top_left).rounded_tr(r.top_right).rounded_br(r.bottom_right).rounded_bl(r.bottom_left)
}

/// Raios da miniatura da grade com o hover em `t` (0–1): em repouso estreito
/// em cima à esquerda e embaixo à direita; no hover cheio os cantos trocam.
fn petal_corners(t: f32) -> Corners<Pixels> {
    let (a, b) = (lerp(PETAL_TIGHT, PETAL_WIDE, t), lerp(PETAL_WIDE, PETAL_TIGHT, t));
    Corners { top_left: px(a), top_right: px(b), bottom_right: px(a), bottom_left: px(b) }
}

/// Colunas da lista: metade das da grade (um card da lista vale dois da grade).
fn list_columns(grid_cols: usize) -> usize {
    (grid_cols / 2).max(1)
}

/// Linha de cima do card da lista: "EPISÓDIO 1 · 29 SET 2023".
fn kicker(label: &str, date: Option<&str>) -> String {
    let ep = label.strip_prefix('E').filter(|n| !n.is_empty()).map(|n| format!("Episódio {n}"));
    [ep, date.map(str::to_owned)].into_iter().flatten().collect::<Vec<_>>().join(" · ").to_uppercase()
}

/// Número grande do card da grade: "E1" vira "01". Sem número, nada.
fn head_number(label: &str) -> Option<String> {
    label.strip_prefix('E')?.parse::<u32>().ok().map(|n| format!("{n:02}"))
}

/// Imagem da miniatura: still do TMDB (pedido só quando aparece na tela) com
/// `zoom`, esqueleto enquanto o TMDB responde ou fundo neutro sem dados. O
/// recorte do pai é retangular, então os cantos vão em cada camada.
fn still(it: &Item, loading: bool, radii: Corners<Pixels>, zoom: f32, cx: &mut App) -> AnyElement {
    match (&it.still, loading) {
        (_, true) => corners(Skeleton::new().size_full(), radii).into_any_element(),
        (None, false) => corners(neutral(cx), radii).into_any_element(),
        (Some(url), false) => match state(cx).read(cx).cached_image(url) {
            Some(Img::Ready(image)) => div()
                .size_full()
                .child(zoomed_image(image, zoom, radii))
                .with_animation(SharedString::from(format!("img-in:{url}")), enter(QUICK), |d, t| d.opacity(t))
                .into_any_element(),
            Some(Img::Failed) => corners(neutral(cx), radii).into_any_element(),
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
    }
}

/// Imagem cobrindo a área toda, ampliada `zoom` vezes a partir do centro e
/// recortada na própria área com os cantos `radii`. Pintada à mão porque um
/// `img` maior que o pai perde o arredondamento (o recorte do pai é reto).
fn zoomed_image(image: Arc<gpui_kit::Image>, zoom: f32, radii: Corners<Pixels>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let Some(data) = image.use_render_image(window, cx) else {
                return;
            };
            let cover = ObjectFit::Cover.get_bounds(bounds, data.size(0));
            window.paint_image(bounds, scale_about_center(cover, zoom), radii, data, 0, false).ok();
        },
    )
    .size_full()
}

/// `b` ampliado `k` vezes, com o mesmo centro.
fn scale_about_center(b: Bounds<Pixels>, k: f32) -> Bounds<Pixels> {
    let grown = size(b.size.width * k, b.size.height * k);
    Bounds {
        origin: point(b.origin.x - (grown.width - b.size.width) / 2., b.origin.y - (grown.height - b.size.height) / 2.),
        size: grown,
    }
}

/// Faixa de baixo de um retângulo `w`×`h` de cantos de baixo `bl`/`br`, com
/// `bar` de altura e cheia até `fraction` da largura: os pontos do polígono,
/// seguindo a curva dos cantos (o recorte do pai é reto, e uma barra reta
/// vazaria para fora do canto arredondado).
fn bottom_strip(w: f32, h: f32, bl: f32, br: f32, bar: f32, fraction: f32) -> Vec<(f32, f32)> {
    let end = w * fraction.clamp(0., 1.);
    if end <= 0. || w <= 0. || h <= 0. {
        return Vec::new();
    }
    let limit = (w / 2.).min(h / 2.);
    let (bl, br) = (bl.clamp(0., limit), br.clamp(0., limit));
    // Quanto o canto de raio `r` come da borda na altura `y`.
    let inset = |r: f32, y: f32| {
        let dy = y - (h - r);
        if r <= 0. || dy <= 0. { 0. } else { r - (r * r - dy * dy).max(0.).sqrt() }
    };
    const STEPS: usize = 6;
    let ys: Vec<f32> = (0..=STEPS).map(|i| h - bar + bar * i as f32 / STEPS as f32).collect();
    let left = ys.iter().map(|&y| (inset(bl, y), y));
    let right = ys.iter().rev().map(|&y| ((w - inset(br, y)).min(end).max(inset(bl, y)), y));
    left.chain(right).collect()
}

/// Barra de progresso no pé da miniatura (trilho claro e parte vista na cor da
/// marca), recortada pelos cantos de baixo.
fn progress_strip(fraction: f32, radii: Corners<Pixels>, theme: &Theme) -> impl IntoElement {
    let (bl, br) = (f32::from(radii.bottom_left), f32::from(radii.bottom_right));
    let layers = [(1., theme::media_track(0.3)), (fraction, theme.primary)];
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            for (f, color) in layers {
                let points: Vec<_> = bottom_strip(w, h, bl, br, BAR_HEIGHT, f)
                    .into_iter()
                    .map(|(x, y)| point(bounds.origin.x + px(x), bounds.origin.y + px(y)))
                    .collect();
                if points.len() < 3 {
                    continue;
                }
                let mut path = PathBuilder::fill();
                path.add_polygon(&points, true);
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
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

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(l, n)| (l.to_string(), n.to_string())).collect()
    }

    #[test]
    fn repeated_label_shows_quality() {
        let items = pairs(&[("E1", "Show - 01 [1080p].mkv"), ("E1", "Show - 01 [720p].mkv"), ("E2", "Show - 02 [1080p].mkv")]);
        assert_eq!(disambiguators(&items), [Some("1080p".to_owned()), Some("720p".to_owned()), None]);
    }

    #[test]
    fn repeated_label_with_same_quality_shows_file_name() {
        let items = pairs(&[("E1", "Show - 01 [1080p].mkv"), ("E1", "Show - 01v2 [1080p].mkv")]);
        assert_eq!(disambiguators(&items), [Some("Show - 01 [1080p].mkv".to_owned()), Some("Show - 01v2 [1080p].mkv".to_owned())]);
    }

    #[test]
    fn unique_or_empty_labels_need_nothing() {
        let items = pairs(&[("", "Abertura.mkv"), ("", "Encerramento.mkv"), ("E3", "Show - 03.mkv")]);
        assert_eq!(disambiguators(&items), [None, None, None]);
    }

    #[test]
    fn petal_corners_swap_on_hover() {
        let rest = petal_corners(0.);
        let hover = petal_corners(1.);
        assert_eq!((rest.top_left, rest.top_right, rest.bottom_right, rest.bottom_left), (px(4.), px(22.), px(4.), px(22.)));
        assert_eq!((hover.top_left, hover.top_right, hover.bottom_right, hover.bottom_left), (px(22.), px(4.), px(22.), px(4.)));
        assert_eq!(petal_corners(0.5).top_left, px(13.), "no meio, os dois cantos se encontram");
    }

    #[test]
    fn head_number_pads_the_label() {
        assert_eq!(head_number("E1").as_deref(), Some("01"));
        assert_eq!(head_number("E12").as_deref(), Some("12"));
        assert_eq!(head_number("E105").as_deref(), Some("105"));
        assert_eq!(head_number(""), None);
    }

    #[test]
    fn zoom_keeps_the_center() {
        let b = Bounds { origin: point(px(10.), px(20.)), size: size(px(100.), px(50.)) };
        let z = scale_about_center(b, 1.2);
        let near = |a: Pixels, b: f32| (f32::from(a) - b).abs() < 1e-3;
        assert!(near(z.size.width, 120.) && near(z.size.height, 60.), "{z:?}");
        assert!(near(z.center().x, 60.) && near(z.center().y, 45.), "{z:?}");
        assert_eq!(scale_about_center(b, 1.), b);
    }

    #[test]
    fn strip_without_radius_is_the_bottom_rectangle() {
        let pts = bottom_strip(100., 50., 0., 0., 3., 0.4);
        let xs: Vec<f32> = pts.iter().map(|p| p.0).collect();
        assert!(xs.iter().all(|&x| x == 0. || x == 40.), "{xs:?}");
        assert!(pts.iter().all(|p| (47. ..=50.).contains(&p.1)));
    }

    #[test]
    fn strip_follows_the_rounded_corners() {
        let pts = bottom_strip(200., 100., 22., 4., 3., 1.);
        // Na base, a faixa começa onde a curva do canto de 22 px encosta embaixo.
        let bottom_left = pts.iter().find(|p| p.1 == 100.).unwrap();
        assert!((bottom_left.0 - 22.).abs() < 1e-3, "{bottom_left:?}");
        // E termina antes do canto direito, de 4 px.
        let bottom_right = pts.iter().rev().find(|p| p.1 == 100.).unwrap();
        assert!((bottom_right.0 - 196.).abs() < 1e-3, "{bottom_right:?}");
        assert!(bottom_strip(200., 100., 22., 4., 3., 0.).is_empty(), "nada visto, nada pintado");
    }

    #[test]
    fn list_has_half_the_grid_columns() {
        assert_eq!(list_columns(4), 2);
        assert_eq!(list_columns(5), 2);
        assert_eq!(list_columns(1), 1);
        assert_eq!(list_columns(0), 1);
    }

    #[test]
    fn kicker_names_episode_and_date() {
        assert_eq!(kicker("E1", Some("29 set 2023")), "EPISÓDIO 1 · 29 SET 2023");
        assert_eq!(kicker("E7", None), "EPISÓDIO 7");
        assert_eq!(kicker("", Some("29 set 2023")), "29 SET 2023");
        assert_eq!(kicker("", None), "");
    }

    #[test]
    fn diagonal_cut_opens_on_hover() {
        let b = Bounds { origin: point(px(0.), px(0.)), size: size(px(200.), px(100.)) };
        let [top, bottom, inner] = cut_triangle(b, 1.).unwrap();
        assert_eq!((top, bottom), (point(px(200.), px(0.)), point(px(200.), px(100.))));
        assert!((f32::from(inner.x) - 172.).abs() < 1e-3, "a borda termina a 86 % embaixo: {inner:?}");
        assert!(cut_triangle(b, 0.).is_none(), "aberto no hover cheio");
    }

    #[test]
    fn air_date_like_anilist() {
        assert_eq!(air_date(Some("2023-09-29")).as_deref(), Some("29 set 2023"));
        assert_eq!(air_date(Some("")), None);
        assert_eq!(air_date(None), None);
    }
}
