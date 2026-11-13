//! Início: destaques do ano, continuar, seções do acervo e descobrir.
//!
//! Cada bloco com dados próprios é uma view separada embutida com `cached`, para
//! que a chegada de dados de um bloco não redesenhe os outros.

mod continue_row;
mod discover;
mod hero;

use std::{sync::Arc, time::Duration};

use crate::interact::hover_anim;
use crate::motion::{self, ease_out_cubic};
use gpui_kit::{
    AnimationExt as _,
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, StyleRefinement, Styled, Task, Window,
    assets::IconName, div, prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use higanbana_core::{
    anilist::{MediaType, Ranking, types::RankedMedia},
    catalog::ListBudget,
};

use crate::{
    components::{columns, content_width, section_label, section_meta, section_order},
    state::{AppState, state},
    ui::empty_state,
};

pub struct HomeView {
    state: Entity<AppState>,
    sections: Option<Result<Vec<String>, String>>,
    hero: Entity<hero::Hero>,
    cont: Entity<continue_row::ContinueRow>,
    discover: Entity<discover::Discover>,
}

/// Cor de cada seção da nuvem (tokens do tema): dá identidade aos atalhos.
fn section_hue(name: &str, theme: &gpui_kit::component::Theme) -> gpui_kit::Hsla {
    match name {
        "Animes" => theme.primary,
        "Mangás" => theme.blue,
        "Doramas" => theme.magenta,
        "Desenhos" => theme.yellow,
        "OSTs" => theme.green,
        _ => theme.cyan,
    }
}

/// Título de bloco da entrada: ícone num selo da cor primária + nome.
pub(super) fn eyebrow(icon: IconName, title: impl Into<gpui_kit::SharedString>, cx: &gpui_kit::App) -> gpui_kit::Div {
    let theme = cx.theme();
    h_flex()
        .gap_2p5()
        .child(
            div()
                .size_8()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.primary.opacity(0.14))
                .child(Icon::new(icon).small().text_color(theme.primary)),
        )
        .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(title.into()))
}

/// Obras confirmadas de uma lista (tipo + ranking) e a conferência em andamento.
pub(super) struct Slot {
    pub items: Vec<(RankedMedia, String)>,
    pub done: bool,
    _task: Task<()>,
}

impl Slot {
    pub fn new(task: Task<()>) -> Self {
        Self { items: Vec::new(), done: false, _task: task }
    }
}

/// Ao sair de uma lista que não terminou, cancela a conferência (dropa a `Task`):
/// lista escondida não gasta orçamento. Ao voltar ela recomeça, rápida pelo cache.
pub(super) fn leave<K: Eq + std::hash::Hash>(slots: &mut std::collections::HashMap<K, Slot>, key: &K) {
    if slots.get(key).is_some_and(|s| !s.done) {
        slots.remove(key);
    }
}

/// Cancela a conferência no núcleo quando a view que pediu some.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Confere um ranking no acervo e entrega cada obra confirmada à view, na ordem.
/// Dropar a `Task` cancela a conferência.
#[allow(clippy::too_many_arguments)] // dados da lista + dois callbacks; um struct só esconderia os nomes
pub(super) fn discover_task<V: 'static>(
    state: &Entity<AppState>,
    budget: Arc<ListBudget>,
    media: MediaType,
    ranking: Ranking,
    n: usize,
    cx: &mut Context<V>,
    on_item: impl Fn(&mut V, RankedMedia, String, &mut Context<V>) + 'static,
    on_done: impl FnOnce(&mut V, &mut Context<V>) + 'static,
) -> Task<()> {
    let core = state.read(cx).core.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let c = core.clone();
    let job = core.spawn(async move {
        c.discover(media, ranking, n, budget, move |m, p| {
            let _ = tx.send((m, p));
        })
        .await
    });
    let guard = AbortOnDrop(job.abort_handle());
    cx.spawn(async move |this, cx| {
        let _guard = guard;
        while let Some((m, p)) = rx.recv().await {
            if this.update(cx, |v, cx| on_item(v, m, p, cx)).is_err() {
                return;
            }
        }
        let _ = this.update(cx, on_done);
    })
}

impl HomeView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = state(cx);
        // Só a saudação lê o AppState aqui; os blocos observam por conta própria.
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

        let budget = core.home_budget();
        let hero = cx.new(|cx| hero::Hero::new(budget.clone(), cx));
        let cont = cx.new(continue_row::ContinueRow::new);
        // A altura do Continuar muda com o progresso: a entrada refaz o layout.
        cx.observe(&cont, |_, _, cx| cx.notify()).detach();
        let discover = cx.new(|cx| discover::Discover::new(budget.clone(), cx));
        // Banner primeiro: com o cache frio, as listagens da visita vão para ele.
        let d = discover.clone();
        cx.observe(&hero, move |_, hero, cx| {
            if hero.read(cx).settled() {
                d.update(cx, |d, cx| d.start(cx));
            }
        })
        .detach();
        Self { state, sections: None, hero, cont, discover }
    }

    /// Seções da raiz da nuvem em cartões baixos: ícone e nome.
    fn sections(&self, width: f32, rem: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        // Todas as seções numa linha quando cabem (atalhos baixos, sem sobra solitária).
        let count = self.sections.as_ref().and_then(|r| r.as_ref().ok()).map_or(6, |l| l.len().max(1));
        let cols = columns(width, 9. * rem, 0.75 * rem).min(count) as u16;
        match &self.sections {
            None => div().grid().grid_cols(cols).gap_3().children((0..6).map(|_| Skeleton::new().h_16().rounded(theme.radius_lg))).into_any_element(),
            Some(Err(_)) => {
                empty_state(IconName::WifiOff, "Não foi possível abrir a nuvem", Some("Verifique sua conexão e reabra o app.".into()), cx).into_any_element()
            }
            Some(Ok(list)) => div()
                .grid()
                .grid_cols(cols)
                .gap_3()
                .children(list.iter().enumerate().map(|(i, s)| {
                    let (icon_name, _) = section_meta(s);
                    let target = s.clone();
                    let state = self.state.clone();
                    let hue = section_hue(s, &theme);
                    let label = section_label(s);
                    let id = format!("section:{s}");
                    // Selo na cor da seção; no hover o cartão sobe e a borda pega a cor.
                    let card = hover_anim(ElementId::Name(format!("{id}:hover").into()), Duration::from_millis(200), move |t, _, cx| {
                        let e = ease_out_cubic(t);
                        let theme = cx.theme();
                        h_flex()
                            .gap_3()
                            .h_16()
                            .px_3()
                            .relative()
                            .top(px(-3. * e))
                            .rounded(theme.radius_lg)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.group_box)
                            .when(e > 0.01, |d| d.shadow_md())
                            .child(
                                div()
                                    .size_9()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(theme.radius)
                                    .bg(hue.opacity(0.14 + 0.12 * e))
                                    .child(Icon::new(icon_name).small().text_color(hue)),
                            )
                            .child(div().flex_1().min_w_0().line_clamp(1).font_weight(FontWeight::MEDIUM).child(label.clone()))
                            // Por cima do texto, sem ocupar espaço: só aparece no hover.
                            .child(div().absolute().right_2().top_0().bottom_0().flex().items_center().child(Icon::new(IconName::ChevronRight).small().text_color(theme.muted_foreground.opacity(e))))
                            .child(div().absolute().inset_0().rounded(theme.radius_lg).border_1().border_color(hue.opacity(0.6 * e)))
                    });
                    div()
                        .id(ElementId::Name(id.clone().into()))
                        .cursor_pointer()
                        .child(card)
                        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(target.clone(), cx)))
                        .with_animation(ElementId::Name(format!("{id}:in").into()), motion::staggered(i), |d, t| {
                            d.opacity(t).relative().top(px(motion::RISE_DISTANCE * (1. - t)))
                        })
                }))
                .into_any_element(),
        }
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rem = f32::from(window.rem_size());
        let width = content_width(window, 80. * rem);
        let name = self.state.read(cx).user.as_ref().map(|u| u.name.clone());
        let sections = self.sections(width, rem, cx);

        v_flex()
            .w(px(width))
            .mx_auto()
            .py_8()
            .gap_10()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(match name {
                        Some(n) => format!("Olá, {n}"),
                        None => "Bem-vindo de volta".into(),
                    }))
                    .child(div().text_color(theme.muted_foreground).child("Busque com Ctrl K ou escolha uma seção."))
                    .with_animation("home-greeting", motion::enter(motion::BASE), |d, t| d.opacity(t).relative().top(px(8. * (1. - t)))),
            )
            .child(self.hero.clone().cached(StyleRefinement::default().w_full().h(px(hero::height_rem() * rem))))
            .map(|d| {
                let h = self.cont.read(cx).height_rem();
                if h > 0. { d.child(self.cont.clone().cached(StyleRefinement::default().w_full().h(px(h * rem)))) } else { d }
            })
            .child(v_flex().gap_4().child(eyebrow(IconName::FolderOpen, "Seções", cx)).child(sections))
            .child(self.discover.clone().cached(StyleRefinement::default().w_full().h(px(discover::height_rem() * rem))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_unfinished_list_cancels_it() {
        let mut slots = std::collections::HashMap::new();
        slots.insert("a", Slot::new(Task::ready(())));
        let mut done = Slot::new(Task::ready(()));
        done.done = true;
        slots.insert("b", done);
        leave(&mut slots, &"a");
        leave(&mut slots, &"b");
        assert!(!slots.contains_key("a"));
        assert!(slots.contains_key("b"));
    }
}
