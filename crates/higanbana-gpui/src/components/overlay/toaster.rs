//! Toaster: coloque uma vez na raiz. Empilha os toasts no canto inferior
//! direito, entra deslizando com fade, sai com fade antes de vencer.
use crate::components::basic::Component;
use super::common::{close_button, nothing, panel, sub, sub_i, Lens, PRESENCE};
use super::toast::{Entry, Toasts};
use crate::components::Variant;
use crate::cx::RenderCx;
use crate::element::{div, overlay, text, Anchor, Element, ElementId};
use crate::style::Styled;
use std::rc::Rc;

/// Pinta a fila `Toasts` da view; o "×" usa a lente para fechar.
pub struct Toaster<'a, V> {
    id: ElementId,
    toasts: &'a Toasts,
    lens: Lens<V, Toasts>,
}

impl<'a, V: 'static> Toaster<'a, V> {
    pub fn new(id: impl Into<ElementId>, toasts: &'a Toasts, lens: impl Fn(&mut V) -> &mut Toasts + 'static) -> Self {
        Self { id: id.into(), toasts, lens: Rc::new(lens) }
    }

    fn build(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let now = cx.now();
        if let Some(next) = self.toasts.items.iter().filter(|e| e.expires > now).map(|e| e.expires).min() {
            // Acorda no próximo vencimento (ou no início do fade de saída).
            let fade = PRESENCE.duration;
            let left = next - now;
            cx.request_redraw_in(if left > fade { left - fade } else { left });
        }
        let entries: Vec<&Entry> = self.toasts.visible(now).collect();
        if entries.is_empty() {
            return nothing();
        }
        let mut stack = div().id(self.id).flex_col_reverse().gap(8.0).w(356.0);
        for e in entries {
            stack = stack.child(self.toast(e, cx));
        }
        overlay(Anchor::window(1.0, 1.0), stack).gap(16.0).into()
    }

    fn toast(&self, e: &Entry, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let key = sub_i(self.id, "toast", e.id.0 as usize);
        let enter = cx.animate_from(key, 0.0, 1.0, PRESENCE);
        let fade = PRESENCE.duration.as_secs_f32();
        let exit = ((e.expires - cx.now()).as_secs_f32() / fade).min(1.0);
        let t = enter.min(exit);
        if exit < 1.0 {
            cx.request_animation_frame();
        }
        let theme = cx.theme();
        let destructive = e.toast.variant == Variant::Destructive;
        let (lens, id) = (self.lens.clone(), e.id);
        let close = close_button(sub(key, "fechar"), theme, cx.focus().visible).on_click(move |v: &mut V, _, cx| {
            lens(v).dismiss(id);
            cx.notify();
        });
        let mut body = div().flex_col().gap(4.0).flex_1().child(text(e.toast.title.clone()).font_weight(600));
        if let Some(d) = &e.toast.description {
            body = body.child(text(d.clone()).opacity(0.9).when(!destructive, |t| t.text_color(theme.muted_foreground)));
        }
        let card = panel(theme).id(key).flex_row().items_start().gap(12.0).p(16.0).ml(24.0 * (1.0 - t)).opacity(t).child(body).child(close);
        if destructive {
            card.bg(theme.destructive).border_color(theme.destructive).text_color(theme.destructive_foreground)
        } else {
            card.bg(theme.background)
        }
    }
}

impl<V: 'static> Component<V> for Toaster<'_, V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        self.build(cx)
    }
}
