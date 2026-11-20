//! Peças comuns dos overlays: ids derivados, lentes, presença animada e painel.
use crate::anim::Transition;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element, ElementId};
use crate::style::Styled;
use crate::theme::Theme;
use std::rc::Rc;

/// Lente da view para o estado do componente (`|v| &mut v.campo`).
pub(super) type Lens<V, S> = Rc<dyn Fn(&mut V) -> &mut S>;
/// Callback de mudança de valor.
pub(super) type OnChange<V, T> = Rc<dyn Fn(&mut V, T, &mut EventCx)>;

/// Id filho estável derivado de `id` (sem alocar).
pub(super) fn sub(id: ElementId, part: &str) -> ElementId {
    ElementId(id.0.rotate_left(17) ^ ElementId::from(part).0)
}

/// Id filho indexado (`gatilho 3`, `item 7`...).
pub(super) fn sub_i(id: ElementId, part: &str, i: usize) -> ElementId {
    ElementId(id.0.rotate_left(17) ^ ElementId::from((part, i)).0)
}

/// Transição de entrada/saída dos overlays (150 ms, ease-out).
pub(super) const PRESENCE: Transition = Transition::ms(150);

/// Presença 0..1 de algo que abre/fecha: entra de 0 e sai animando até 0.
/// Enquanto > 0 o componente continua desenhado (animação de saída).
pub(super) fn presence<V>(cx: &mut RenderCx<'_, V>, key: ElementId, open: bool) -> f32 {
    cx.animate_from(key, 0.0, if open { 1.0 } else { 0.0 }, PRESENCE)
}

/// Placeholder invisível para quando nada deve aparecer.
pub(super) fn nothing<V>() -> Element<V> {
    div().hidden()
}

/// Painel flutuante (popover do shadcn): fundo, borda, sombra e raio do tema.
pub(super) fn panel<V>(theme: &Theme) -> Element<V> {
    div()
        .flex_col()
        .bg(theme.popover)
        .text_color(theme.popover_foreground)
        .text_size(theme.typography.sm)
        .border(1.0, theme.border)
        .rounded(theme.radius)
        .shadow(crate::color::rgba(0x0000_0040), 12.0)
}

/// Aplica o anel de foco do tema quando o foco veio do teclado.
pub(super) fn ring<V>(e: Element<V>, theme: &Theme, visible: bool) -> Element<V> {
    let ring = theme.ring;
    if visible { e.focus(move |s| s.border(2.0, ring)) } else { e }
}

/// Botão "×" de fechar (diálogo e toast), focável por `id`.
pub(super) fn close_button<V>(id: ElementId, theme: &Theme, ring_visible: bool) -> Element<V> {
    let fg = theme.foreground;
    let b = div()
        .track_focus(id)
        .size(20.0)
        .items_center()
        .justify_center()
        .rounded(theme.radius / 2.0)
        .text_color(theme.muted_foreground)
        .cursor_pointer()
        .hover(move |s| s.text_color(fg))
        .child(text("×").text_size(theme.typography.base));
    ring(b, theme, ring_visible)
}
