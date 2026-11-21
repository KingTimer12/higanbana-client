//! Botão do shadcn: variantes, tamanhos, ícone à esquerda, foco por teclado.
use super::variant_style::variant_colors;
use super::{focus_ring, Component};
use crate::color::Color;
use crate::components::{ComponentSize, Variant};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, ClickHandler, Element, ElementId, MouseEvent};
use crate::gfx::CursorStyle;
use crate::style::Styled;
use std::borrow::Cow;

/// `Button::new("salvar", "Salvar").variant(Variant::Outline).on_click(..)`.
/// Enter/Espaço ativam o botão focado (infra de foco + `on_click`).
pub struct Button<V> {
    id: ElementId,
    label: Cow<'static, str>,
    variant: Variant,
    size: ComponentSize,
    icon: Option<Element<V>>,
    disabled: bool,
    full_width: bool,
    on_click: Option<ClickHandler<V>>,
}

impl<V> Button<V> {
    pub fn new(id: impl Into<ElementId>, label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: Variant::Default,
            size: ComponentSize::Md,
            icon: None,
            disabled: false,
            full_width: false,
            on_click: None,
        }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn size(mut self, s: ComponentSize) -> Self {
        self.size = s;
        self
    }

    /// Elemento à esquerda do rótulo (ícone).
    pub fn icon(mut self, icon: impl Into<Element<V>>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Ocupa toda a largura do pai.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut V, &MouseEvent, &mut EventCx) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

/// Padding horizontal e gap por tamanho (px-3/4/6, gap-1.5/2).
fn metrics(size: ComponentSize) -> (f32, f32) {
    match size {
        ComponentSize::Sm => (12.0, 6.0),
        ComponentSize::Md => (16.0, 8.0),
        ComponentSize::Lg => (24.0, 8.0),
        ComponentSize::Icon => (0.0, 0.0),
    }
}

impl<V: 'static> Component<V> for Button<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let theme = cx.theme();
        let c = variant_colors(theme, self.variant);
        let (pad, gap) = metrics(self.size);
        let h = self.size.height();
        let ring = theme.ring;
        let shadowed = !matches!(self.variant, Variant::Ghost | Variant::Link);
        let mut e = div()
            .track_focus(self.id)
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(gap)
            .h(h)
            .px(pad)
            .rounded(theme.radius - 2.0)
            .bg(c.bg)
            .border(1.0, c.border)
            .text_color(c.fg)
            .text_size(theme.typography.sm)
            .font_weight(500)
            .whitespace_nowrap()
            .when(shadowed, |e| e.shadow(Color::BLACK.alpha(0.05), 2.0))
            .when(self.size == ComponentSize::Icon, |e| e.w(h))
            .when(self.full_width, |e| e.w_full());
        if self.disabled {
            e = e.opacity(0.5).cursor(CursorStyle::NotAllowed).disabled(true);
        } else {
            let (hb, hf) = (c.hover_bg, c.hover_fg);
            e = e.cursor_pointer().hover(move |s| s.bg(hb).text_color(hf));
            if cx.focus().visible {
                e = e.focus(move |s| focus_ring(s, ring));
            }
        }
        e.on_click = self.on_click;
        e.children(self.icon).when(!self.label.is_empty(), |e| e.child(text(self.label)))
    }
}
