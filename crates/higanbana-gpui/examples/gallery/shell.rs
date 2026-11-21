//! Moldura da galeria: cabeçalho (tema + backend), barra lateral e rodapé de estatísticas.
use crate::sections::SECTIONS;
use crate::Gallery;
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;

/// Barra de título customizada: logo, nome, Select de backend (troca em tempo real),
/// botão de tema e os botões da janela.
pub fn header(v: &Gallery, cx: &mut RenderCx<'_, Gallery>) -> Element<Gallery> {
    let kinds: Vec<BackendKind> = cx.backends().iter().map(|b| b.kind).collect();
    let mut state = v.backend_select;
    state.selected = kinds.iter().position(|k| *k == cx.backend());
    let labels: Vec<String> = kinds.iter().map(|k| k.label().to_string()).collect();
    let select = Select::new("backend", &state, |v: &mut Gallery| &mut v.backend_select)
        .items(labels)
        .width(140.0)
        .on_change(move |_, i, cx| {
            if let Some(k) = kinds.get(i) {
                cx.switch_backend(*k);
            }
        })
        .render(cx);
    let next = cx.theme().toggled();
    let label = if cx.theme().dark { "Tema claro" } else { "Tema escuro" };
    let toggle = Button::new("tema", label).variant(Variant::Outline).on_click(move |_, _, cx| cx.set_theme(next.clone())).render(cx);
    let version = Badge::new("v0.1").variant(Variant::Secondary).render(cx);
    let theme = cx.theme();
    let logo = div().size(24.0).rounded(6.0).bg(rgb(0xdc2626)).items_center().justify_center().child(text("彼").text_size(13.0).text_color(rgb(0xffffff)));
    let actions = div().items_center().gap(10.0).child(text("Backend").text_size(14.0).text_color(theme.muted_foreground)).child(select).child(toggle);
    TitleBar::new("Higanbana UI").leading(logo).content(version).trailing(actions).render(cx)
}

/// Lista rolável de seções agrupadas; a ativa fica destacada.
pub fn sidebar(v: &Gallery, cx: &RenderCx<'_, Gallery>) -> Element<Gallery> {
    let theme = cx.theme();
    let mut list = scroll("secoes").w(220.0).flex_shrink_0().p(12.0).gap(2.0);
    let mut group = "";
    for (i, s) in SECTIONS.iter().enumerate() {
        if s.group != group {
            group = s.group;
            let top = if i == 0 { 4.0 } else { 16.0 };
            list = list.child(text(group).text_size(12.0).font_weight(600).text_color(theme.muted_foreground).px(10.0).pt(top).pb(4.0).flex_shrink_0());
        }
        let active = i == v.section;
        let hover = theme.accent;
        list = list.child(
            div()
                .id(("secao", i))
                .flex_shrink_0()
                .px(10.0)
                .py(6.0)
                .rounded(6.0)
                .text_size(14.0)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .when(active, |e| e.bg(theme.accent).text_color(theme.accent_foreground).font_weight(600))
                .when(!active, |e| e.text_color(theme.muted_foreground))
                .on_click(move |v: &mut Gallery, _, cx| {
                    v.section = i;
                    cx.notify();
                })
                .child(s.title),
        );
    }
    list
}

/// Estatísticas do último quadro e o backend/adaptador ativos.
pub fn footer(cx: &RenderCx<'_, Gallery>) -> Element<Gallery> {
    let theme = cx.theme();
    let s = cx.stats();
    let adapter = cx.backends().iter().find(|b| b.kind == cx.backend()).map(|b| b.adapter.clone()).unwrap_or_default();
    let item = |label: String| text(label).text_size(12.0).text_color(theme.muted_foreground);
    div()
        .h(28.0)
        .px(16.0)
        .gap(20.0)
        .flex_shrink_0()
        .items_center()
        .bg(theme.card)
        .child(item(format!("entre quadros {:.1} ms", s.frame_ms)))
        .child(item(format!("GPU {:.1} MB", s.gpu_bytes as f64 / 1_048_576.0)))
        .child(item(format!("{} draws", s.draw_calls)))
        .child(item(if adapter.is_empty() { cx.backend().label().to_string() } else { format!("{} · {}", cx.backend().label(), adapter) }))
}
