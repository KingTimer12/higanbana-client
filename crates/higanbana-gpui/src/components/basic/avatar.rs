//! Avatar do shadcn: imagem circular, ou iniciais sobre `muted` quando não há imagem.
use super::Component;
use crate::components::ComponentSize;
use crate::cx::RenderCx;
use crate::element::{div, img, text, Element, ImageSource};
use crate::style::Styled;
use std::borrow::Cow;

/// `Avatar::new("Ana Beatriz").image(src)`; sem imagem mostra "AB".
pub struct Avatar {
    name: Cow<'static, str>,
    image: Option<ImageSource>,
    size: ComponentSize,
}

impl Avatar {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self { name: name.into(), image: None, size: ComponentSize::Md }
    }

    pub fn image(mut self, src: impl Into<Option<ImageSource>>) -> Self {
        self.image = src.into();
        self
    }

    /// Sm = 24, Md/Icon = 32 (size-8 do shadcn), Lg = 40 px.
    pub fn size(mut self, s: ComponentSize) -> Self {
        self.size = s;
        self
    }
}

/// Até duas iniciais maiúsculas: primeira e última palavra ("ana de souza" -> "AS").
pub fn initials(name: &str) -> String {
    let mut words = name.split_whitespace().filter_map(|w| w.chars().next());
    let first = words.next();
    let last = words.last();
    first.into_iter().chain(last).flat_map(char::to_uppercase).collect()
}

impl<V> Component<V> for Avatar {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let theme = cx.theme();
        let (d, font) = match self.size {
            ComponentSize::Sm => (24.0, theme.typography.xs),
            ComponentSize::Md | ComponentSize::Icon | ComponentSize::Regular => (32.0, theme.typography.sm),
            ComponentSize::Lg => (40.0, theme.typography.base),
            // Kit: xsmall 16 px, small 24 px.
            ComponentSize::Xs => (16.0, theme.typography.xs),
            ComponentSize::Compact => (24.0, theme.typography.xs),
        };
        let shell = div().size(d).flex_shrink_0().rounded_full().overflow_hidden();
        match self.image {
            Some(src) => shell.child(img(src).size_full().rounded_full()),
            None => shell
                .items_center()
                .justify_center()
                .bg(theme.muted)
                .text_color(theme.foreground)
                .text_size(font)
                .font_weight(500)
                .child(text(initials(&self.name))),
        }
    }
}
