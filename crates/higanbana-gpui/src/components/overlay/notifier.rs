//! Notifier: notificações no visual do kit sobre a mesma fila `Toasts`.
//! Topo-direito abaixo da barra de título, 382 px, fundo popover, ícone pelo tom.
//! Entram deslizando 96 px da direita (400 ms) e saem com fade (200 ms).
use super::common::{nothing, sub_i, Lens};
use super::toast::{Entry, Toasts};
use crate::anim::{Easing, Transition};
use crate::color::Color;
use crate::components::basic::Component;
use crate::cx::RenderCx;
use crate::element::{div, overlay, text, Anchor, Element, ElementId};
use crate::icon::icon;
use crate::style::Styled;
use std::rc::Rc;
use std::time::Duration;

/// Altura da barra de título do kit (as notificações ficam abaixo dela).
const TITLE_BAR: f32 = 34.0;
const MARGIN: f32 = 16.0;
const WIDTH: f32 = 382.0;
const SLIDE: f32 = 96.0;
const ENTER: Transition = Transition::ms(400).easing(Easing::EaseOut);
const EXIT: Duration = Duration::from_millis(200);

/// Coloque uma vez na raiz: `Notifier::new("avisos", &v.toasts, |v| &mut v.toasts)`.
pub struct Notifier<'a, V> {
    id: ElementId,
    toasts: &'a Toasts,
    _lens: Lens<V, Toasts>,
}

impl<'a, V: 'static> Notifier<'a, V> {
    pub fn new(id: impl Into<ElementId>, toasts: &'a Toasts, lens: impl Fn(&mut V) -> &mut Toasts + 'static) -> Self {
        Self { id: id.into(), toasts, _lens: Rc::new(lens) }
    }

    fn card(&self, e: &Entry, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let key = sub_i(self.id, "aviso", e.id.0 as usize);
        let enter = cx.animate_from(key, 0.0, 1.0, ENTER);
        let exit = ((e.expires - cx.now()).as_secs_f32() / EXIT.as_secs_f32()).min(1.0);
        if exit < 1.0 {
            cx.request_animation_frame();
        }
        let t = cx.theme();
        let mut card = div()
            .id(key)
            .relative()
            .w_full()
            .flex_col()
            .py(14.0)
            .px(16.0)
            .gap(4.0)
            .border(1.0, t.border)
            .bg(t.popover)
            .text_color(t.popover_foreground)
            .rounded(t.radius_lg)
            .shadow(Color::BLACK.alpha(0.12), 10.0)
            .text_size(t.typography.sm)
            .ml(SLIDE * (1.0 - enter))
            .opacity(enter.min(exit));
        if let Some(tone) = e.toast.tone {
            let (name, color) = tone.look(t);
            card = card.pl(16.0 + 24.0).child(div().absolute().top(18.0).left(16.0).child(icon(name).size(16.0).text_color(color)));
        }
        let msg = e.toast.description.clone();
        card = card.child(text(e.toast.title.clone()).font_weight(if msg.is_some() { 600 } else { 400 }));
        match msg {
            Some(m) => card.child(text(m)),
            None => card,
        }
    }
}

impl<V: 'static> Component<V> for Notifier<'_, V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let now = cx.now();
        if let Some(next) = self.toasts.items.iter().filter(|e| e.expires > now).map(|e| e.expires).min() {
            let left = next - now;
            cx.request_redraw_in(if left > EXIT { left - EXIT } else { left });
        }
        let entries: Vec<&Entry> = self.toasts.visible(now).collect();
        if entries.is_empty() {
            return nothing();
        }
        // Mais recente em cima, abaixo da barra de título (sem faixa transparente engolindo cliques).
        let mut stack = div().id(self.id).flex_col().gap(8.0).w(WIDTH);
        for e in entries {
            stack = stack.child(self.card(e, cx));
        }
        overlay(Anchor::window(1.0, 0.0), stack).gap(MARGIN).offset(0.0, TITLE_BAR).into()
    }
}
