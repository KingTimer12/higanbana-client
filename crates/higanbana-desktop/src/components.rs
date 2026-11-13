//! Peças das telas do acervo: capa, pôster, migalhas, lista de arquivos.
//!
//! Tudo lê cor de `cx.theme()`; as únicas cores fixas são as de sobre-mídia
//! (`theme::on_media*`) e a cor que o AniList dá para cada capa (é dado).

use crate::interact::Button;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{
    breadcrumb::{Breadcrumb, BreadcrumbItem},
    button::ButtonVariants as _,
    h_flex,
    progress::Progress,
    tag::Tag,
    v_flex, ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _,
};
use gpui_kit::{
    assets::IconName, canvas, div, img, prelude::*, px, relative, Corners, AnimationExt as _, AnyElement, App,
    Bounds, Div, ElementId, FontWeight, Hsla, IntoElement, MouseButton, ObjectFit, ParentElement, Pixels,
    SharedString, Stateful, Styled, Window,
};
use higanbana_core::{
    anilist::{text, types::MediaSummary, MediaType},
    library::{folder_title, folder_year, join_path, natural_cmp, parse_episode, raw_path, split_path, VersionLabel},
    nuvem::FileEntry,
    player::logic::VIDEO_EXT,
};

use crate::{
    interact::{hover_fade, hover_layer},
    state::{state, AppState, DlStatus, Img},
    theme::{self, ProductColors as _},
    ui::truncate,
};

/// Chama `f` (fora do desenho) quando o elemento entra na área visível, com folga.
pub fn when_visible(margin: f32, f: impl FnOnce(&mut App) + 'static) -> impl IntoElement {
    canvas(
        move |bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App| {
            let view = window.viewport_size();
            let top = f32::from(bounds.origin.y);
            let bottom = top + f32::from(bounds.size.height);
            if bottom >= -margin && top <= f32::from(view.height) + margin {
                cx.defer(f);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// Imagem de uma URL pública; vazio enquanto carrega (o fundo do pai aparece).
/// Quando chega, aparece com um fade curto em vez de "estourar" na tela.
pub fn remote_image(url: Option<&str>, cx: &mut App) -> AnyElement {
    rounded_image(url, None, cx)
}

/// Como `remote_image`, recortando os cantos: o recorte do pai não arredonda imagens.
pub fn rounded_image(url: Option<&str>, radius: Option<Pixels>, cx: &mut App) -> AnyElement {
    let Some(url) = url else {
        return div().size_full().into_any_element();
    };
    let s = state(cx);
    match s.update(cx, |s, cx| s.image(url, cx)) {
        Img::Ready(image) => img(image)
            .size_full()
            .object_fit(ObjectFit::Cover)
            .when_some(radius, |i, r| i.rounded(r))
            .with_animation(
                SharedString::from(format!("img-in:{url}")),
                crate::motion::enter(crate::motion::QUICK),
                |i, t| i.opacity(t),
            )
            .into_any_element(),
        _ => div().size_full().into_any_element(),
    }
}

/// Cor dominante da capa no AniList (`#rrggbb`). É dado da obra, não do tema.
pub fn hex_color(c: Option<&str>) -> Option<Hsla> {
    let c = c?.trim_start_matches('#');
    u32::from_str_radix(c, 16)
        .ok()
        .filter(|_| c.len() == 6)
        .map(|v| gpui_kit::rgb(v).into())
}

/// Capa tipográfica para títulos sem correspondência no AniList: o nome
/// inteiro (quebrado em linhas, nunca cortado no meio) sobre a cor de superfície.
fn type_poster(name: &str, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .size_full()
        .justify_between()
        .p_3()
        .bg(theme.muted)
        .child(Icon::new(IconName::Clapperboard).small().text_color(theme.muted_foreground))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(relative(1.2))
                .text_color(theme.foreground.opacity(0.8))
                .line_clamp(5)
                .child(folder_title(name)),
        )
}

/// Capa 2:3 do AniList ou tipográfica. `width` vem da grade medida.
pub fn cover(media: Option<&MediaSummary>, name: &str, width: Option<Pixels>, cx: &mut App) -> Div {
    let bg = media
        .and_then(|m| hex_color(m.color()))
        .unwrap_or(cx.theme().muted);
    let base = div()
        .relative()
        .overflow_hidden()
        .rounded(cx.theme().radius_lg)
        .bg(bg);
    let base = match width {
        Some(w) => base.w(w).h(w * 1.5),
        None => base.w_full(),
    };
    let radius = cx.theme().radius_lg;
    let key = media.and_then(|m| state(cx).update(cx, |s, scx| s.cover_key(m, false, scx)));
    match key {
        Some(url) => base.child(rounded_image(Some(&url), Some(radius), cx)),
        None => base.child(type_poster(name, cx)),
    }
}

/// Marca de "está no acervo", sobre a capa.
pub fn in_library_mark(cx: &App) -> Div {
    div()
        .absolute()
        .top_2()
        .left_2()
        .size_6()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().primary)
        .child(
            Icon::new(IconName::FolderCheck)
                .xsmall()
                .text_color(cx.theme().primary_foreground),
        )
}

/// Nota do AniList sobre a capa.
fn score_mark(score: i64, cx: &App) -> Div {
    h_flex()
        .absolute()
        .top_2()
        .right_2()
        .gap_1()
        .px_1p5()
        .py_0p5()
        .rounded(cx.theme().radius)
        .bg(theme::media_scrim(0.7))
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::on_media())
        .child(
            Icon::new(IconName::Star)
                .xsmall()
                .text_color(cx.theme().yellow),
        )
        .child(score.to_string())
}

pub struct PosterProps {
    pub id: ElementId,
    pub name: String,
    pub media: Option<MediaSummary>,
    pub loading: bool,
    pub width: Pixels,
    pub badge: bool,
    pub dimmed: bool,
    /// Linha de baixo; `None` mostra formato · ano.
    pub meta: Option<String>,
}

/// Card de pôster (capa + título + formato/ano). O clique vem de quem usa.
pub fn poster(p: PosterProps, cx: &mut App) -> Stateful<Div> {
    let title = folder_title(&p.name);
    let media = p.media.as_ref();
    let year = media
        .and_then(|m| m.year())
        .or_else(|| folder_year(&p.name).map(|y| y as i32));
    let format = media
        .and_then(|m| m.format.as_deref())
        .map(|f| text::format(Some(f)));
    let cover_el = if p.loading {
        Skeleton::new()
            .w(p.width)
            .h(p.width * 1.5)
            .rounded(cx.theme().radius_lg)
            .into_any_element()
    } else {
        cover(media, &p.name, Some(p.width), cx).into_any_element()
    };
    let meta = p.meta.clone().unwrap_or_else(|| {
        [
            format.filter(|f| !f.is_empty()),
            year.map(|y| y.to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
    });
    let theme = cx.theme().clone();
    v_flex()
        .id(p.id)
        .gap_2()
        .w(p.width)
        .cursor_pointer()
        .when(p.dimmed, |d| d.opacity(0.55))
        .child(
            hover_fade(
                "cover-ring",
                div()
                    .relative()
                    .rounded(theme.radius_lg)
                    .child(cover_el)
                    .when(p.badge, |d| d.child(in_library_mark(cx)))
                    .children(
                        media
                            .and_then(|m| m.average_score)
                            .map(|s| score_mark(s, cx)),
                    ),
            )
            .border(theme.ring)
            .radius(theme.radius_lg),
        )
        .child(
            v_flex()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_clamp(2)
                        .child(title),
                )
                .when(!meta.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(meta),
                    )
                }),
        )
}

/// Colunas que cabem em `width` com itens de pelo menos `min` e espaço `gap` (pixels medidos).
pub fn columns(width: f32, min: f32, gap: f32) -> usize {
    (((width + gap) / (min + gap)).floor() as usize).max(1)
}

/// Largura útil do conteúdo: janela − barra lateral − margens da página, no máximo `max`.
pub fn content_width(window: &Window, max: f32) -> f32 {
    let rem = f32::from(window.rem_size());
    // Barra lateral do kit (255 px) + margens da página (px_8 dos dois lados).
    (f32::from(window.viewport_size().width) - 255. - 4. * rem).clamp(200., max)
}

/// Rótulo de seção na barra lateral, no início e nas migalhas.
pub fn section_label(name: &str) -> String {
    match name {
        "[#Temp]" => "Recentes".into(),
        "AniGUI (Downloader Oficial)" => "AniGUI".into(),
        n => n.into(),
    }
}

/// Ícone e descrição de cada seção da raiz.
pub fn section_meta(name: &str) -> (IconName, &'static str) {
    match name {
        "Animes" => (IconName::Clapperboard, "Séries, filmes e OVAs, por letra."),
        "Mangás" => (IconName::BookOpen, "Séries completas e volumes avulsos."),
        "Doramas" => (IconName::Tv, "Dramas asiáticos com legenda."),
        "Desenhos" => (IconName::Palette, "Animações ocidentais."),
        "OSTs" => (IconName::Music, "Trilhas de animes, jogos e filmes."),
        "[#Temp]" => (IconName::Inbox, "Envios novos, antes de organizar."),
        _ => (IconName::FolderOpen, ""),
    }
}

/// Ordem na barra lateral: conteúdo principal primeiro.
pub fn section_order(a: &str, b: &str) -> std::cmp::Ordering {
    const ORDER: [&str; 7] = [
        "Animes",
        "Mangás",
        "Doramas",
        "Desenhos",
        "OSTs",
        "[#Temp]",
        "AniGUI (Downloader Oficial)",
    ];
    let pos = |s: &str| ORDER.iter().position(|o| *o == s).unwrap_or(99);
    pos(a).cmp(&pos(b)).then_with(|| natural_cmp(a, b))
}

/// Rota de uma subpasta: dentro de uma pasta aberta crua (`raw:`), continua crua.
pub fn folder_link(dir: &str, name: &str, raw: bool) -> String {
    let path = join_path(dir, name);
    if raw { raw_path(&path) } else { path }
}

/// Migalhas: Início › Seção › ... › atual.
pub fn breadcrumbs(path: &str, cx: &mut App) -> Breadcrumb {
    breadcrumbs_in(path, false, cx)
}

/// Migalhas de uma pasta; com `raw`, cada pedaço abre cru (o Início não).
pub fn breadcrumbs_in(path: &str, raw: bool, cx: &mut App) -> Breadcrumb {
    let segs: Vec<String> = split_path(path).into_iter().map(str::to_owned).collect();
    let s = state(cx);
    let go = |target: String| {
        let s = s.clone();
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            s.update(cx, |s, cx| s.go(target.clone(), cx))
        }
    };
    let mut crumbs =
        Breadcrumb::new().child(BreadcrumbItem::new("Início").on_click(go(String::new())));
    for (i, seg) in segs.iter().enumerate() {
        let label = if i == 0 {
            section_label(seg)
        } else {
            seg.clone()
        };
        crumbs = crumbs.child(BreadcrumbItem::new(label).on_click(go(folder_link(&segs[..i].join("/"), seg, raw))));
    }
    crumbs
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024. && i < UNITS.len() - 1 {
        v /= 1024.;
        i += 1;
    }
    let num = if i >= 3 {
        format!("{v:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    } else {
        format!("{v:.0}")
    };
    format!("{} {}", num.replace('.', ","), UNITS[i])
}

pub fn is_comic(f: &FileEntry) -> bool {
    !f.is_directory
        && f.extension
            .as_deref()
            .is_some_and(|e| matches!(e.to_lowercase().as_str(), ".cbz" | ".zip" | ".cbr" | ".rar"))
}

/// Área de botões dentro de uma linha ou card clicável: o clique (e o aperto)
/// para aqui e não chega ao pai. O `Button` do kit não para a propagação, e os
/// estados sem botão (baixando, "Baixado") também não podem tocar o episódio.
pub fn stop_click(id: impl Into<ElementId>, child: impl IntoElement) -> Stateful<Div> {
    div()
        .id(id)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(child)
}

/// Ação "Baixar" de um arquivo, mostrando o estado do download no mesmo lugar.
pub fn download_button(id: impl Into<ElementId>, path: &str, cx: &mut App) -> AnyElement {
    let id = id.into();
    let s = state(cx);
    let d = s.read(cx).download(path).cloned();
    let path = path.to_owned();
    let start = {
        let s = s.clone();
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            s.update(cx, |s, cx| {
                s.start_downloads(std::slice::from_ref(&path), cx)
            })
        }
    };
    match d.map(|d| (d.status, d)) {
        Some((DlStatus::Queued, _)) => Button::new(id)
            .ghost()
            .small()
            .label("Na fila")
            .disabled(true)
            .into_any_element(),
        Some((DlStatus::Running, d)) => v_flex()
            .w_20()
            .gap_1()
            .items_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{}%", (d.progress() * 100.).round()))
            .child(
                Progress::new(id)
                    .value(d.progress() * 100.)
                    .accessibility_label("Baixando"),
            )
            .into_any_element(),
        Some((DlStatus::Done, _)) => h_flex()
            .gap_1()
            .px_2()
            .text_xs()
            .text_color(cx.theme().success_foreground)
            .child(Icon::new(IconName::Check).xsmall())
            .child("Baixado")
            .into_any_element(),
        Some((DlStatus::Error, _)) => Button::new(id)
            .ghost()
            .small()
            .icon(IconName::RefreshCw)
            .label("Tentar de novo")
            .on_click(start)
            .into_any_element(),
        None => Button::new(id)
            .ghost()
            .small()
            .icon(IconName::Download)
            .tooltip("Baixar para o computador")
            .on_click(start)
            .into_any_element(),
    }
}

/// Caminhos dos arquivos (não pastas) de uma listagem.
pub fn file_paths(dir: &str, files: &[FileEntry]) -> Vec<String> {
    files
        .iter()
        .filter(|f| !f.is_directory)
        .map(|f| join_path(dir, &f.name))
        .collect()
}

pub type FileAction = std::rc::Rc<dyn Fn(&FileEntry, &mut Window, &mut App)>;

/// Lista de uma pasta: subpastas primeiro, depois arquivos como episódios.
///
/// Colunas fixas em toda linha: número, nome e detalhes, ações.
pub fn file_list(
    dir: &str,
    files: &[FileEntry],
    on_play: Option<FileAction>,
    on_read: Option<FileAction>,
    cx: &mut App,
) -> Div {
    file_list_in(dir, files, false, on_play, on_read, cx)
}

/// [`file_list`] com `raw`: as subpastas abrem cruas, como a pasta atual.
pub fn file_list_in(
    dir: &str,
    files: &[FileEntry],
    raw: bool,
    on_play: Option<FileAction>,
    on_read: Option<FileAction>,
    cx: &mut App,
) -> Div {
    let mut sorted = files.to_vec();
    sorted.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
    if sorted.is_empty() {
        return crate::ui::empty_state(IconName::FolderOpen, "Esta pasta está vazia", None, cx)
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_dashed()
            .border_color(cx.theme().border);
    }
    // Quando todos os episódios têm a mesma qualidade, mostra uma vez só no topo.
    let eps: Vec<_> = sorted
        .iter()
        .filter(|f| !f.is_directory)
        .map(|f| parse_episode(&f.name))
        .filter(|e| e.number.is_some())
        .collect();
    let shared = (eps.len() >= 2)
        .then(|| eps[0].quality.clone())
        .flatten()
        .filter(|q| eps.iter().all(|e| e.quality.as_deref() == Some(q)));

    let s = state(cx);
    let (progress, reading) = {
        let st = s.read(cx);
        (
            st.player_data.progress.clone(),
            crate::reader::progress_map(st),
        )
    };
    let theme = cx.theme().clone();

    let mut list = v_flex()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border)
        .bg(theme.group_box)
        .overflow_hidden();
    let total = sorted.len();
    for (i, f) in sorted.into_iter().enumerate() {
        let path = join_path(dir, &f.name);
        // Identidade pelo nome do arquivo, não pela posição.
        let key = SharedString::from(f.name.clone());
        let row = h_flex()
            .gap_3()
            .px_3()
            .py_2p5()
            .when(i > 0, |d| d.border_t_1().border_color(theme.border));
        // Fundo do hover acompanha os cantos da lista na primeira e na última linha.
        let r = theme.radius_lg;
        let corners = Corners {
            top_left: if i == 0 { r } else { px(0.) },
            top_right: if i == 0 { r } else { px(0.) },
            bottom_left: if i + 1 == total { r } else { px(0.) },
            bottom_right: if i + 1 == total { r } else { px(0.) },
        };
        if f.is_directory {
            let s = s.clone();
            let path = folder_link(dir, &f.name, raw);
            list = list.child(
                hover_fade(
                    ElementId::Name(format!("dir-hover:{key}").into()),
                    row.id(ElementId::Name(format!("dir:{key}").into()))
                        .cursor_pointer()
                        .child(
                            div().w_10().flex_none().flex().justify_center().child(
                                Icon::new(IconName::Folder).text_color(theme.muted_foreground),
                            ),
                        )
                        .child(truncate(f.name.clone()).flex_1().text_sm())
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .small()
                                .text_color(theme.muted_foreground),
                        )
                        .on_click(move |_, _, cx| s.update(cx, |s, cx| s.go(path.clone(), cx))),
                )
                .fill(theme.list_hover)
                .radius(corners),
            );
            continue;
        }
        let ep = parse_episode(&f.name);
        let label = match (&ep.number, &shared) {
            (None, _) => f.name.clone(),
            (Some(n), Some(_)) => format!(
                "Episódio {}",
                n.parse::<f64>().map_or(n.clone(), |v| v.to_string())
            ),
            (Some(_), None) => ep.quality.clone().unwrap_or(ep.label.clone()),
        };
        let ext = f.extension.clone().unwrap_or_default().to_lowercase();
        let playable = on_play.is_some() && VIDEO_EXT.contains(&ext.as_str());
        let readable = on_read.is_some() && is_comic(&f);
        let read_p = if readable {
            reading.get(&path).cloned()
        } else {
            None
        };
        let (watched, frac) = match (&read_p, progress.get(&path)) {
            (Some(r), _) => (r.read, (r.page as f64 + 1.) / (r.total.max(1) as f64)),
            (None, Some(p)) => (p.watched, p.position / p.duration.max(1.)),
            _ => (false, -1.),
        };
        let details = [
            ext.trim_start_matches('.').to_uppercase(),
            format_bytes(f.size),
        ]
        .join(" · ");
        // Clicar na linha faz a ação principal (assistir ou ler); a área dos botões (`stop_click`) não repassa o clique.
        let primary: Option<FileAction> = if playable { on_play.clone() } else if readable { on_read.clone() } else { None };
        let mut row = row
            .id(ElementId::Name(format!("ep:{key}").into()))
            .when_some(primary, |d, act| {
                let f2 = f.clone();
                d.cursor_pointer().on_click(move |_, w, cx| act(&f2, w, cx))
            })
            .child(
                div()
                    .w_10()
                    .flex_none()
                    .flex()
                    .justify_center()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(match &ep.number {
                        // Visto: o número dá lugar ao check; o resto continua legível.
                        Some(_) if watched => Icon::new(IconName::Check)
                            .text_color(theme.success_foreground)
                            .into_any_element(),
                        Some(n) => div()
                            .text_color(if frac >= 0. { theme.primary_ink() } else { theme.muted_foreground })
                            .child(n.clone())
                            .into_any_element(),
                        None => Icon::new(IconName::FilePlay)
                            .text_color(theme.muted_foreground)
                            .into_any_element(),
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(truncate(label).text_sm().font_weight(FontWeight::MEDIUM))
                    .child(
                        h_flex()
                            .gap_3()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(details)
                            .when(watched, |d| {
                                d.child(
                                    h_flex()
                                        .gap_1()
                                        .text_color(theme.success_foreground)
                                        .child(Icon::new(IconName::Check).xsmall())
                                        .child(if readable { "Lido" } else { "Visto" }),
                                )
                            }),
                    )
                    .when(frac >= 0. && !watched, |d| {
                        d.child(
                            div().pt_1().w_48().child(
                                Progress::new(ElementId::Name(format!("seen:{key}").into()))
                                    .xsmall()
                                    .value((frac.min(1.) * 100.) as f32),
                            ),
                        )
                    }),
            );
        if playable {
            let (act, f2) = (on_play.clone().unwrap(), f.clone());
            row = row.child(stop_click(
                ElementId::Name(format!("play-area:{key}").into()),
                Button::new(ElementId::Name(format!("play:{key}").into()))
                    .ghost()
                    .small()
                    .icon(IconName::Play)
                    .label("Assistir")
                    .on_click(move |_, w, cx| act(&f2, w, cx)),
            ));
        }
        if readable {
            let (act, f2) = (on_read.clone().unwrap(), f.clone());
            let label = if read_p.as_ref().is_some_and(|r| !r.read) {
                "Continuar"
            } else {
                "Ler"
            };
            row = row.child(stop_click(
                ElementId::Name(format!("read-area:{key}").into()),
                Button::new(ElementId::Name(format!("read:{key}").into()))
                    .ghost()
                    .small()
                    .icon(IconName::BookOpen)
                    .label(label)
                    .on_click(move |_, w, cx| act(&f2, w, cx)),
            ));
        }
        row = row.child(stop_click(
            ElementId::Name(format!("dl-area:{key}").into()),
            div().w_20().flex().justify_end().child(download_button(
                ElementId::Name(format!("dl:{key}").into()),
                &path,
                cx,
            )),
        ));
        list = list.child(
            hover_fade(ElementId::Name(format!("ep-hover:{key}").into()), row)
                .fill(theme.list_hover)
                .radius(corners),
        );
    }
    let mut out = v_flex().gap_2();
    if let Some(q) = shared {
        out = out.child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(format!("Todos os episódios em {q}")),
        );
    }
    out.child(list)
}

/// Grupo de opções com rótulo à esquerda (temporada, versão).
pub fn option_group(label: &'static str, cx: &App) -> Div {
    h_flex().items_start().gap_4().child(
        div()
            .w_20()
            .flex_none()
            .pt_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(label),
    )
}

/// Uma opção de um grupo (temporada, versão): botão com estado selecionado.
pub fn option_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> Button {
    Button::new(id)
        .small()
        .outline()
        .label(label)
        .selected(selected)
}

/// Cartão de uma versão (release) da obra: nome em destaque e etiquetas de
/// resolução, fonte, áudio e episódios. O clique vem de quem usa.
pub fn version_card(
    id: impl Into<ElementId>,
    label: &VersionLabel,
    episodes: Option<usize>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let tags = [label.resolution.clone(), label.source.clone(), label.audio.clone(), episodes.map(|n| format!("{n} ep"))];
    v_flex()
        .id(id)
        .relative()
        .min_w_40()
        .max_w_80()
        .gap_1()
        .px_3()
        .py_2()
        .rounded(theme.radius_lg)
        .border_1()
        .cursor_pointer()
        .map(|d| {
            if selected {
                d.border_color(theme.primary).bg(theme.primary.opacity(0.1))
            } else {
                d.border_color(theme.border).bg(theme.muted)
            }
        })
        .child(truncate(label.name.clone()).text_sm().font_weight(FontWeight::SEMIBOLD))
        .child(
            h_flex()
                .flex_wrap()
                .gap_1()
                .children(tags.into_iter().flatten().map(|t| Tag::secondary().xsmall().child(t))),
        )
        .when(!selected, |d| d.child(hover_layer("ring").border(theme.ring).radius(theme.radius_lg)))
}

/// Pôster de um título da nuvem numa grade: busca os metadados quando aparece
/// na tela e abre `target` ao clicar.
pub fn title_card(id: ElementId, name: &str, target: String, media: MediaType, width: Pixels, cx: &mut App) -> Div {
    let s = state(cx);
    let looked = s.read(cx).lookup(name, media);
    let card = poster(
        PosterProps {
            id,
            name: name.to_owned(),
            media: looked.clone().flatten(),
            loading: looked.is_none(),
            width,
            badge: false,
            dimmed: false,
            meta: None,
        },
        cx,
    )
    .on_click(move |_, _, cx| s.update(cx, |s, cx| s.go(target.clone(), cx)));
    div()
        .relative()
        .child(card)
        .when(looked.is_none(), |d| d.child(when_visible(400., want_lookup(name.to_owned(), media))))
}

pub fn want_lookup(name: String, media: MediaType) -> impl FnOnce(&mut App) {
    move |cx| {
        state(cx).update(cx, |s: &mut AppState, cx| s.want_lookup(&name, media, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_link_keeps_raw() {
        assert_eq!(folder_link("Animes/Letra S", "Smile", false), "Animes/Letra S/Smile");
        assert_eq!(folder_link("Animes/Letra S", "Smile", true), "raw:Animes/Letra S/Smile");
        assert_eq!(folder_link("", "Animes", true), "raw:Animes");
    }
}
