//! Badge do shadcn: rótulo pequeno (text-xs, rounded-md) com variantes.
use super::variant_style::variant_colors;
use super::Component;
use crate::color::Color;
use crate::components::Variant;
use crate::cx::RenderCx;
use crate::element::{div, text, Element};
use crate::style::Styled;
use std::borrow::Cow;

/// `Badge::new("Novo").variant(Variant::Secondary)`. Variantes do shadcn:
/// Default, Secondary, Outline, Destructive (Ghost/Link usam o visual de Outline).
pub struct Badge {
    label: Cow<'static, str>,
    variant: Variant,
}

impl Badge {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        Self { label: label.into(), variant: Variant::Default }
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
}

impl<V> Component<V> for Badge {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let theme = cx.theme();
        let variant = match self.variant {
            Variant::Ghost | Variant::Link => Variant::Outline,
            v => v,
        };
        let mut c = variant_colors(theme, variant);
        if variant == Variant::Outline {
            c.bg = Color::TRANSPARENT;
            c.border = theme.border;
        }
        div()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .px(8.0)
            .py(2.0)
            .rounded(theme.radius - 2.0)
            .bg(c.bg)
            .border(1.0, c.border)
            .text_color(c.fg)
            .text_size(theme.typography.xs)
            .font_weight(500)
            .whitespace_nowrap()
            .child(text(self.label))
    }
}
