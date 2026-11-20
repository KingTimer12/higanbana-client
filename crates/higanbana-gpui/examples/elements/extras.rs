//! Peças interativas do exemplo: campo de texto, tooltip (overlay ancorado) e
//! diálogo modal com fundo escurecido, Escape/clique fora e animação de entrada.
use super::{button, Showcase};
use higanbana_gpui::prelude::*;

/// Campo de busca + botão com tooltip ao passar o mouse.
pub fn form(v: &Showcase, cx: &RenderCx<'_, Showcase>) -> Element<Showcase> {
    let theme = cx.theme();
    let ring = theme.ring;
    let field = text_field("busca", &v.search, |v: &mut Showcase| &mut v.search)
        .w(260.0)
        .px(10.0)
        .py(7.0)
        .rounded(theme.radius - 2.0)
        .bg(theme.background)
        .border(1.0, theme.input)
        .focus(move |s| s.border(1.0, ring))
        .caret_color(theme.primary);
    let tip = overlay(Anchor::above("abrir").align(AnchorAlign::Center), tooltip("Abre um diálogo modal", theme)).gap(6.0);
    let open = button("Abrir diálogo".into(), theme, true)
        .track_focus("abrir")
        .cursor_pointer()
        .focus(move |s| s.border(2.0, ring))
        .on_hover(|v: &mut Showcase, on, cx| {
            v.tip = on;
            cx.notify();
        })
        .on_click(|v: &mut Showcase, _, cx| {
            v.dialog = true;
            v.tip = false;
            cx.notify();
        })
        .when(v.tip, |e| e.child(tip));
    div().gap(12.0).items_center().child(field).child(open)
}

fn tooltip(label: &'static str, theme: &Theme) -> Element<Showcase> {
    div().px(8.0).py(4.0).rounded(6.0).bg(theme.popover).border(1.0, theme.border).text_color(theme.popover_foreground).text_size(12.0).child(label)
}

/// Diálogo centralizado; entra com fade. Fecha no Escape, clique fora ou botão.
pub fn dialog(v: &Showcase, cx: &mut RenderCx<'_, Showcase>) -> Option<Element<Showcase>> {
    if !v.dialog {
        return None;
    }
    let fade = cx.animate_from("dialogo", 0.0, 1.0, Transition::ms(180));
    let theme = cx.theme();
    let close = |v: &mut Showcase, cx: &mut EventCx| {
        v.dialog = false;
        cx.notify();
    };
    let panel = div()
        .flex_col()
        .w(360.0)
        .p(20.0)
        .gap(12.0)
        .rounded(theme.radius)
        .bg(theme.card)
        .border(1.0, theme.border)
        .shadow(rgba(0x0000_0080), 24.0)
        .opacity(fade)
        .child(text("Diálogo").text_size(18.0).font_bold())
        .child(text(format!("Busca atual: “{}”", v.search.text())).text_color(theme.muted_foreground))
        .child(div().justify_end().child(button("Fechar".into(), theme, false).cursor_pointer().on_click(move |v, _, cx| close(v, cx))));
    Some(overlay(Anchor::center(), panel).modal().backdrop(rgba(0x0000_0099).fade(fade)).on_dismiss(close).into())
}
