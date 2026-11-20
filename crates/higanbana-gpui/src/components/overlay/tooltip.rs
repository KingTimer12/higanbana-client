//! Tooltip (shadcn): balão pequeno em `primary` que aparece após um atraso de
//! hover (timer, sem polling) e some ao sair. Lado configurável.
use super::common::{presence, sub, Lens};
use crate::cx::{RenderCx, TimerId};
use crate::element::{div, overlay, text, Anchor, AnchorAlign, Element, ElementId, Side};
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;
use std::time::Duration;

/// Estado do tooltip na view: visível e o timer pendente do atraso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TooltipState {
    pub visible: bool,
    pub(crate) timer: Option<TimerId>,
}

/// Envolve `trigger` e mostra `label` depois de `delay` com o mouse em cima.
pub struct Tooltip<V> {
    id: ElementId,
    visible: bool,
    lens: Lens<V, TooltipState>,
    trigger: Element<V>,
    label: Cow<'static, str>,
    side: Side,
    delay: Duration,
}

impl<V: 'static> Tooltip<V> {
    pub fn new(
        id: impl Into<ElementId>,
        state: &TooltipState,
        lens: impl Fn(&mut V) -> &mut TooltipState + 'static,
        trigger: impl Into<Element<V>>,
        label: impl Into<Cow<'static, str>>,
    ) -> Self {
        let (id, visible) = (id.into(), state.visible);
        Self { id, visible, lens: Rc::new(lens), trigger: trigger.into(), label: label.into(), side: Side::Top, delay: Duration::from_millis(500) }
    }

    /// Lado do balão (padrão: em cima; vira se não couber).
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Atraso até aparecer (padrão 500 ms).
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }

    pub fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = presence(cx, sub(self.id, "dica"), self.visible);
        let theme = cx.theme();
        let (lens, delay, id) = (self.lens, self.delay, self.id);
        let wrapper = div().id(id).on_hover(move |v: &mut V, on, cx| {
            let st = lens(v);
            if let Some(old) = st.timer.take() {
                cx.cancel_timer(old);
            }
            if on {
                let l = lens.clone();
                st.timer = Some(cx.spawn_timer(delay, move |v: &mut V, cx| {
                    let st = l(v);
                    st.timer = None;
                    st.visible = true;
                    cx.notify();
                }));
            } else if st.visible {
                st.visible = false;
                cx.notify();
            }
        });
        let wrapper = wrapper.child(self.trigger);
        if t <= 0.001 {
            return wrapper;
        }
        let bubble = div()
            .id(sub(id, "balao"))
            .px(12.0)
            .py(6.0)
            .rounded(theme.radius - 2.0)
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .text_size(theme.typography.xs)
            .whitespace_nowrap()
            .opacity(t)
            .child(text(self.label));
        let anchor = match self.side {
            Side::Top => Anchor::above(id),
            Side::Bottom => Anchor::below(id),
            Side::Left => Anchor::left_of(id),
            Side::Right => Anchor::right_of(id),
        };
        wrapper.child(overlay(anchor.align(AnchorAlign::Center), bubble).gap(4.0))
    }
}
