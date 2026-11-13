//! Moldura da janela: barra de título, busca (paleta), downloads e barra lateral.

use crate::interact::Button;
use std::time::Duration;

use gpui_kit::{
    App, Context, Entity, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Task,
    Window, assets::IconName, div, prelude::*,
};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IndexPath, Sizable as _, WindowExt as _, h_flex, v_flex,
    avatar::Avatar,
    button::ButtonVariants as _,
    command::{Command, CommandGroup, CommandItem, CommandState},
    kbd::Kbd,
    popover::Popover,
    progress::Progress,
    sidebar::{Sidebar, SidebarFooter, SidebarMenu, SidebarMenuItem},
};
use higanbana_core::{
    library::{media_for_kind, raw_path, split_path},
    nuvem::{SearchKind, SearchResult},
};

use crate::{
    components::{format_bytes, rounded_image, section_label, section_meta, section_order},
    state::{AppState, DlStatus, state},
    ui::{empty_state, truncate},
};

// --- Busca ---

const DEBOUNCE: Duration = Duration::from_millis(250);
/// Quantos resultados pedem capa ao AniList.
const THUMBS: usize = 8;

/// Paleta de busca do acervo (Ctrl K). Os resultados vêm da nuvem, então a
/// paleta não filtra sozinha: cada consulta pede uma busca nova.
pub struct SearchPalette {
    state: Entity<AppState>,
    command: Entity<CommandState>,
    query: String,
    /// Resultados agrupados por tipo, na ordem em que aparecem na paleta.
    groups: Vec<(SearchKind, Vec<SearchResult>)>,
    debounce: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

fn kind_heading(k: SearchKind) -> &'static str {
    match k {
        SearchKind::Anime => "Animes",
        SearchKind::Manga => "Mangás",
        SearchKind::Dorama => "Doramas",
        SearchKind::Ost => "OSTs",
        SearchKind::Desenho => "Desenhos",
        SearchKind::Folder => "Pastas",
        SearchKind::Other => "Outros",
    }
}

const KIND_ORDER: [SearchKind; 7] =
    [SearchKind::Anime, SearchKind::Manga, SearchKind::Dorama, SearchKind::Desenho, SearchKind::Ost, SearchKind::Folder, SearchKind::Other];

impl SearchPalette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        let command = cx.new(|cx| CommandState::new(window, cx));
        let subs = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        Self { state, command, query: String::new(), groups: Vec::new(), debounce: None, _subs: subs }
    }

    /// Abre a paleta num diálogo, com o foco no campo de busca.
    pub fn open(this: &Entity<Self>, window: &mut Window, cx: &mut App) {
        if window.has_active_dialog(cx) {
            return;
        }
        let palette = this.clone();
        window.open_dialog(cx, move |dialog, window, _| {
            let palette = palette.clone();
            dialog.close_button(false).p_0().w(window.rem_size() * 40.).content(move |content, _, _| content.child(palette.clone()))
        });
        let command = this.read(cx).command.clone();
        command.update(cx, |c, cx| c.focus(window, cx));
    }

    fn on_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        let q = query.trim().to_owned();
        self.query = q.clone();
        if q.is_empty() {
            self.groups.clear();
            self.debounce = None;
            self.command.update(cx, |c, cx| c.set_loading(false, window, cx));
            cx.notify();
            return;
        }
        self.command.update(cx, |c, cx| c.set_loading(true, window, cx));
        self.debounce = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let _ = this.update_in(cx, |v, window, cx| v.search(q, window, cx));
        }));
    }

    fn search(&mut self, q: String, window: &mut Window, cx: &mut Context<Self>) {
        let core = self.state.read(cx).core.clone();
        let asked = q.clone();
        let task = core.clone().spawn(async move { core.cloud().search(&q).await });
        self.debounce = Some(cx.spawn_in(window, async move |this, cx| {
            let results = task.await.ok().and_then(Result::ok).map(|r| r.results).unwrap_or_default();
            let _ = this.update_in(cx, |v, window, cx| {
                // Resposta de uma consulta antiga: descarta.
                if v.query != asked {
                    return;
                }
                v.set_results(results, cx);
                v.command.update(cx, |c, cx| c.set_loading(false, window, cx));
                cx.notify();
            });
        }));
    }

    fn set_results(&mut self, results: Vec<SearchResult>, cx: &mut Context<Self>) {
        let mut groups: Vec<(SearchKind, Vec<SearchResult>)> = Vec::new();
        for kind in KIND_ORDER {
            let items: Vec<SearchResult> = results.iter().filter(|r| r.kind == kind).cloned().collect();
            if !items.is_empty() {
                groups.push((kind, items));
            }
        }
        // Capas do AniList para os primeiros resultados.
        let wanted: Vec<_> = groups
            .iter()
            .flat_map(|(_, items)| items.iter())
            .filter_map(|r| media_for_kind(r.kind).map(|m| (r.name.clone(), m)))
            .take(THUMBS)
            .collect();
        self.state.update(cx, |s, cx| {
            for (name, media) in wanted {
                s.want_lookup(&name, media, cx);
            }
        });
        self.groups = groups;
    }

    fn confirm(&mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.groups.get(ix.section).and_then(|(_, items)| items.get(ix.row)).cloned() else { return };
        window.close_dialog(cx);
        self.state.update(cx, |s, cx| s.go(r.path, cx));
    }

    fn result_item(&self, r: &SearchResult, cx: &mut App) -> CommandItem {
        let thumb = media_for_kind(r.kind)
            .and_then(|m| self.state.read(cx).lookup(&r.name, m).flatten())
            .and_then(|m| self.state.update(cx, |s, scx| s.cover_key(&m, true, scx)));
        let parent = split_path(&r.path).iter().rev().skip(1).rev().copied().collect::<Vec<_>>().join(" / ");
        let name = r.name.clone();
        CommandItem::new().label(name.clone()).child(move |_, cx| {
            let theme = cx.theme().clone();
            h_flex()
                .w_full()
                .gap_3()
                .py_1()
                .child(
                    div()
                        .w_8()
                        .h_12()
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .rounded(theme.radius)
                        .bg(theme.muted)
                        .child(match &thumb {
                            Some(url) => rounded_image(Some(url), Some(theme.radius), cx),
                            None => Icon::new(IconName::Folder).small().text_color(theme.muted_foreground).into_any_element(),
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(truncate(name.clone()).font_weight(FontWeight::MEDIUM))
                        .child(truncate(parent.clone()).text_xs().text_color(theme.muted_foreground)),
                )
        })
    }
}

impl Render for SearchPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (on_query, on_confirm) = (cx.weak_entity(), cx.weak_entity());
        let mut command = Command::new(&self.command)
            .bordered(false)
            .filterable(false)
            .max_h(gpui_kit::rems(28.))
            .placeholder("Buscar animes, mangás, OSTs…")
            .on_query(move |q, window, cx| {
                let _ = on_query.update(cx, |v, cx| v.on_query(q, window, cx));
            })
            .on_confirm(move |ix, window, cx| {
                let _ = on_confirm.update(cx, |v, cx| v.confirm(ix, window, cx));
            });
        for (kind, items) in &self.groups {
            let group = CommandGroup::new().label(kind_heading(*kind)).items(items.iter().map(|r| self.result_item(r, cx)).collect::<Vec<_>>());
            command = command.group(group);
        }
        let query = self.query.clone();
        command.empty(move |_, _, cx| {
            if query.is_empty() {
                empty_state(IconName::Search, "Busque pelo nome da pasta", Some("Funciona melhor com o título em romaji, como “Shingeki no Kyojin”.".into()), cx)
            } else {
                empty_state(IconName::Search, format!("Nada encontrado para “{query}”"), Some("Tente o título em romaji ou uma parte menor do nome.".into()), cx)
            }
        })
    }
}

/// Botão da barra de título que parece um campo e abre a paleta.
fn search_trigger(palette: &Entity<SearchPalette>, cx: &App) -> impl IntoElement {
    let palette = palette.clone();
    Button::new("search")
        .outline()
        .small()
        .w_96()
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::Search).small())
                .child(div().flex_1().text_left().child("Buscar animes, mangás, OSTs…"))
                .child(Kbd::new(gpui_kit::Keystroke::parse("ctrl-k").expect("atalho válido"))),
        )
        .on_click(move |_, window, cx| SearchPalette::open(&palette, window, cx))
}

// --- Downloads ---

fn downloads_panel(state: &Entity<AppState>, cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let items = state.read(cx).downloads.clone();
    let any_finished = items.iter().any(|d| matches!(d.status, DlStatus::Done | DlStatus::Error));
    let body = if items.is_empty() {
        empty_state(IconName::Download, "Nenhum download ainda", Some("Abra um título e escolha os episódios para baixar.".into()), cx).into_any_element()
    } else {
        v_flex()
            .id("downloads-list")
            .max_h_96()
            .overflow_y_scroll()
            .children(items.into_iter().map(|d| {
                let (ic, color) = match d.status {
                    DlStatus::Done => (IconName::Check, theme.success_foreground),
                    DlStatus::Error => (IconName::TriangleAlert, theme.danger_foreground),
                    _ => (IconName::Download, theme.muted_foreground),
                };
                let status = match d.status {
                    DlStatus::Queued => "Na fila".to_owned(),
                    DlStatus::Running => match d.total {
                        Some(t) => format!("{} de {}", format_bytes(d.downloaded), format_bytes(t)),
                        None => format_bytes(d.downloaded),
                    },
                    DlStatus::Done => "Concluído".into(),
                    DlStatus::Error => d.error.clone().unwrap_or_else(|| "Falhou".into()),
                };
                let key = SharedString::from(d.id.clone());
                let action = match d.status {
                    DlStatus::Done => d.dest.clone().map(|dest| {
                        Button::new(gpui_kit::ElementId::Name(format!("reveal:{key}").into()))
                            .ghost()
                            .small()
                            .icon(IconName::FolderOpen)
                            .tooltip("Mostrar na pasta")
                            .on_click(move |_, _, cx| cx.reveal_path(&dest))
                            .into_any_element()
                    }),
                    DlStatus::Error => {
                        let (st, path) = (state.clone(), d.path.clone());
                        Some(
                            Button::new(gpui_kit::ElementId::Name(format!("retry:{key}").into()))
                                .ghost()
                                .small()
                                .icon(IconName::RefreshCw)
                                .tooltip("Tentar de novo")
                                .on_click(move |_, _, cx| st.update(cx, |s, cx| s.start_downloads(std::slice::from_ref(&path), cx)))
                                .into_any_element(),
                        )
                    }
                    _ => None,
                };
                h_flex()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .child(Icon::new(ic).small().text_color(color))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(truncate(d.name.clone()).text_sm().font_weight(FontWeight::MEDIUM))
                            .when(matches!(d.status, DlStatus::Running | DlStatus::Queued), |x| {
                                x.child(Progress::new(gpui_kit::ElementId::Name(format!("dl-progress:{key}").into())).xsmall().value(d.progress() * 100.))
                            })
                            .child(div().text_xs().text_color(if d.status == DlStatus::Error { theme.danger_foreground } else { theme.muted_foreground }).child(status)),
                    )
                    .children(action)
            }))
            .into_any_element()
    };
    v_flex()
        .w_96()
        .child(
            h_flex()
                .justify_between()
                .px_3()
                .pb_2()
                .border_b_1()
                .border_color(theme.border)
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Downloads"))
                .when(any_finished, |d| {
                    let st = state.clone();
                    d.child(Button::new("clear-downloads").ghost().xsmall().label("Limpar concluídos").on_click(move |_, _, cx| st.update(cx, |s, cx| s.clear_finished(cx))))
                }),
        )
        .child(div().pt_1().child(body))
}

fn downloads_menu(state: &Entity<AppState>, cx: &App) -> impl IntoElement {
    let (pending, overall) = {
        let s = state.read(cx);
        (s.pending_downloads(), s.overall_progress())
    };
    let st = state.clone();
    let trigger = Button::new("downloads")
        .ghost()
        .small()
        .icon(IconName::Download)
        .tooltip(if pending > 0 { format!("Downloads: {pending} em andamento ({}%)", (overall * 100.).round()) } else { "Downloads".into() })
        .when(pending > 0, |b| b.label(pending.to_string()));
    Popover::new("downloads-popover").anchor(gpui_kit::Anchor::TopRight).trigger(trigger).content(move |_, _, cx| downloads_panel(&st, cx).into_any_element())
}

// --- Barra de título ---

pub fn title_bar(state: &Entity<AppState>, palette: &Entity<SearchPalette>, cx: &App) -> impl IntoElement {
    use gpui_kit::component::TitleBar;
    let (can_back, can_fwd) = {
        let s = state.read(cx);
        (s.can_back(), s.can_forward())
    };
    let nav = |id: &'static str, icon: IconName, tip: &'static str, enabled: bool, back: bool| {
        let st = state.clone();
        Button::new(id).ghost().small().icon(icon).tooltip(tip).disabled(!enabled).on_click(move |_, _, cx| {
            st.update(cx, |s, cx| if back { s.back(cx) } else { s.forward(cx) })
        })
    };
    TitleBar::new().child(
        h_flex()
            .w_full()
            .h_full()
            .pr_2()
            .gap_1()
            .child(div().font_weight(FontWeight::BOLD).text_color(cx.theme().primary).child("higanbana"))
            .child(div().pr_3().text_xs().text_color(cx.theme().muted_foreground).child(format!("v{}", higanbana_core::version::current())))
            .child(nav("back", IconName::ChevronLeft, "Voltar (Alt+←)", can_back, true))
            .child(nav("forward", IconName::ChevronRight, "Avançar (Alt+→)", can_fwd, false))
            .child(h_flex().flex_1().justify_center().child(search_trigger(palette, cx)))
            .child(update_button(state, cx))
            .child(downloads_menu(state, cx)),
    )
}

// --- Barra lateral ---

/// Seções da raiz em ordem de leitura.
pub fn sort_sections(mut names: Vec<String>) -> Vec<String> {
    names.sort_by(|a, b| section_order(a, b));
    names
}

pub fn sidebar(state: &Entity<AppState>, sections: &[String], cx: &mut App) -> impl IntoElement {
    let s = state.read(cx);
    // `raw:Animes/...` continua dentro da seção Animes.
    let raw = raw_path("");
    let path = s.path().strip_prefix(raw.as_str()).unwrap_or(s.path());
    let current = split_path(path).first().map(|s| s.to_string()).unwrap_or_default();
    let user = s.user.clone();
    let core = s.core.clone();
    let go = |target: String| {
        let st = state.clone();
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| st.update(cx, |s, cx| s.go(target.clone(), cx))
    };
    let mut menu = SidebarMenu::new().child(SidebarMenuItem::new("Início").cursor_pointer().icon(IconName::House).active(current.is_empty()).on_click(go(String::new())));
    for name in sections {
        let (icon, _) = section_meta(name);
        menu = menu.child(SidebarMenuItem::new(section_label(name)).cursor_pointer().icon(icon).active(current == *name).on_click(go(name.clone())));
    }
    let avatar = user.as_ref().and_then(|u| u.avatar.clone());
    let name = user.map(|u| u.name).unwrap_or_default();
    let footer = SidebarFooter::new().child(
        h_flex()
            .w_full()
            .gap_2()
            .child(match avatar {
                Some(url) => div().size_8().flex_none().rounded_full().overflow_hidden().bg(cx.theme().muted).child(rounded_image(Some(&url), Some(gpui_kit::px(999.)), cx)).into_any_element(),
                None => Avatar::new().name(name.clone()).small().into_any_element(),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(truncate(name).text_sm().font_weight(FontWeight::SEMIBOLD))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Via Discord")),
            )
            .child(Button::new("logout").ghost().small().icon(IconName::LogOut).tooltip("Sair da conta").on_click(move |_, _, _| {
                let c = core.clone();
                core.spawn(async move { c.auth().logout().await });
            })),
    );
    Sidebar::new("nav").collapsible(false).child(menu).footer(footer)
}

/// Aviso de versão nova na barra de título (some quando não há atualização).
fn update_button(state: &Entity<AppState>, cx: &App) -> impl IntoElement {
    use crate::updater::UpdateState;
    let (label, tip, busy) = match &state.read(cx).update {
        UpdateState::Idle => return div().into_any_element(),
        UpdateState::Available(r) => (format!("Atualizar para v{}", r.version), "Baixar e instalar a nova versão (o app reinicia)".to_string(), false),
        UpdateState::Downloading(r) => (format!("Baixando v{}...", r.version), "Baixando e conferindo a atualização".to_string(), true),
    };
    let st = state.clone();
    Button::new("update")
        .primary()
        .small()
        .icon(IconName::Download)
        .label(label)
        .tooltip(tip)
        .disabled(busy)
        .on_click(move |_, _, cx| st.update(cx, |s, cx| s.install_update(cx)))
        .into_any_element()
}
