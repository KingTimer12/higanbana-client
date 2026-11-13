//! Início: destaques do ano, continuar, seções do acervo e descobrir.
//!
//! Cada bloco com dados próprios é uma view separada embutida com `cached`, para
//! que a chegada de dados de um bloco não redesenhe os outros.

mod continue_row;
mod hero;

use std::sync::Arc;

use crate::interact::hover_layer;
use gpui_kit::{
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

/// Listagens da nuvem que uma visita à entrada pode gastar conferindo o acervo.
const LIST_BUDGET: u32 = 20;

pub struct HomeView {
    state: Entity<AppState>,
    sections: Option<Result<Vec<String>, String>>,
    hero: Entity<hero::Hero>,
    cont: Entity<continue_row::ContinueRow>,
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

        let budget = ListBudget::new(LIST_BUDGET);
        let hero = cx.new(|cx| hero::Hero::new(budget.clone(), cx));
        let cont = cx.new(continue_row::ContinueRow::new);
        // A altura do Continuar muda com o progresso: a entrada refaz o layout.
        cx.observe(&cont, |_, _, cx| cx.notify()).detach();
        Self { state, sections: None, hero, cont }
    }

    /// Seções da raiz da nuvem em cartões baixos: ícone e nome.
    fn sections(&self, width: f32, rem: f32, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let cols = columns(width, 11. * rem, 0.75 * rem) as u16;
        match &self.sections {
            None => div().grid().grid_cols(cols).gap_3().children((0..6).map(|_| Skeleton::new().h_16().rounded(theme.radius_lg))).into_any_element(),
            Some(Err(_)) => {
                empty_state(IconName::WifiOff, "Não foi possível abrir a nuvem", Some("Verifique sua conexão e reabra o app.".into()), cx).into_any_element()
            }
            Some(Ok(list)) => div()
                .grid()
                .grid_cols(cols)
                .gap_3()
                .children(list.iter().map(|s| {
                    let (icon_name, _) = section_meta(s);
                    let target = s.clone();
                    let state = self.state.clone();
                    h_flex()
                        .id(ElementId::Name(format!("section:{s}").into()))
                        .gap_3()
                        .h_16()
                        .px_3()
                        .rounded(theme.radius_lg)
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.group_box)
                        .relative()
                        .cursor_pointer()
                        .child(
                            div()
                                .size_8()
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(theme.radius)
                                .bg(theme.muted)
                                .child(Icon::new(icon_name).small().text_color(theme.foreground)),
                        )
                        .child(div().flex_1().min_w_0().line_clamp(1).font_weight(FontWeight::MEDIUM).child(section_label(s)))
                        .child(hover_layer("ring").border(theme.ring).radius(theme.radius_lg))
                        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(target.clone(), cx)))
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
                    .child(div().text_color(theme.muted_foreground).child("Busque com Ctrl K ou escolha uma seção.")),
            )
            .child(self.hero.clone().cached(StyleRefinement::default().w_full().h(px(hero::height_rem() * rem))))
            .map(|d| {
                let h = self.cont.read(cx).height_rem();
                if h > 0. { d.child(self.cont.clone().cached(StyleRefinement::default().w_full().h(px(h * rem)))) } else { d }
            })
            .child(v_flex().gap_4().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Seções")).child(sections))
    }
}
