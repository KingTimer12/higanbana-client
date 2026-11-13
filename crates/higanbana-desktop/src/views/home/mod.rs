//! Início: destaques do ano, continuar, seções do acervo e descobrir.
//!
//! Cada bloco com dados próprios é uma view separada embutida com `cached`, para
//! que a chegada de dados de um bloco não redesenhe os outros.

use std::sync::Arc;

use crate::interact::hover_layer;
use gpui_kit::{
    Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Window, assets::IconName, div,
    prelude::*, px,
};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use higanbana_core::catalog::ListBudget;

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
    /// Compartilhado pelos blocos que conferem o acervo.
    #[allow(dead_code)]
    budget: Arc<ListBudget>,
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

        Self { state, sections: None, budget: ListBudget::new(LIST_BUDGET) }
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
            .child(v_flex().gap_4().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Seções")).child(sections))
    }
}
