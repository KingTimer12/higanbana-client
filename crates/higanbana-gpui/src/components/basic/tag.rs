//! Tag do kit: etiqueta de gênero/assunto (secondary ou outline), tamanhos small e xsmall.
use super::Component;
use crate::color::Color;
use crate::cx::RenderCx;
use crate::element::{div, text, Element};
use crate::style::Styled;
use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Secondary,
    Outline,
}

/// `Tag::secondary("Ação").small()`.
pub struct Tag {
    label: Cow<'static, str>,
    look: Look,
    small: bool,
    xsmall: bool,
}

impl Tag {
    pub fn secondary(label: impl Into<Cow<'static, str>>) -> Self {
        Tag { label: label.into(), look: Look::Secondary, small: false, xsmall: false }
    }

    pub fn outline(label: impl Into<Cow<'static, str>>) -> Self {
        Tag { look: Look::Outline, ..Tag::secondary(label) }
    }

    /// `small` do kit: texto 10 px, px 6 / py 2, raio = metade do tema.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// `xsmall` do kit (mesmas medidas do small).
    pub fn xsmall(mut self) -> Self {
        self.xsmall = true;
        self
    }
}

impl<V> Component<V> for Tag {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = cx.theme();
        let (bg, fg, border) = match self.look {
            Look::Secondary => (t.secondary, t.secondary_foreground, t.secondary),
            Look::Outline => (Color::TRANSPARENT, t.foreground, t.border),
        };
        let compact = self.small || self.xsmall;
        let (size, px, py, radius) = if compact { (10.0, 6.0, 2.0, t.radius / 2.0) } else { (t.typography.xs, 10.0, 4.0, t.radius) };
        div()
            .flex_shrink_0()
            .items_center()
            .px(px)
            .py(py)
            .rounded(radius)
            .bg(bg)
            .border(1.0, border)
            .text_color(fg)
            .text_size(size)
            .whitespace_nowrap()
            .child(text(self.label))
    }
}
