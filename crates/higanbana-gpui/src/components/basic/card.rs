//! Card do shadcn: caixa com borda, fundo `card`, sombra leve e slots
//! cabeçalho (título + descrição), conteúdo e rodapé.
use super::Component;
use crate::color::Color;
use crate::cx::RenderCx;
use crate::element::{div, text, Element};
use crate::style::Styled;
use std::borrow::Cow;

/// Cabeçalho: título (semibold) e descrição opcional (muted, text-sm).
pub struct CardHeader {
    title: Cow<'static, str>,
    description: Option<Cow<'static, str>>,
}

impl CardHeader {
    pub fn new(title: impl Into<Cow<'static, str>>) -> Self {
        Self { title: title.into(), description: None }
    }

    pub fn description(mut self, d: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(d.into());
        self
    }
}

/// `Card::new().header(CardHeader::new("Título")).content(..).footer(..)`.
/// Layout: coluna com py-6 e gap-6; cada slot tem px-6.
pub struct Card<V> {
    header: Option<CardHeader>,
    content: Option<Element<V>>,
    footer: Option<Element<V>>,
}

impl<V> Default for Card<V> {
    fn default() -> Self {
        Self { header: None, content: None, footer: None }
    }
}

impl<V> Card<V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn header(mut self, h: CardHeader) -> Self {
        self.header = Some(h);
        self
    }

    pub fn content(mut self, e: impl Into<Element<V>>) -> Self {
        self.content = Some(e.into());
        self
    }

    /// Rodapé em linha, itens centralizados verticalmente.
    pub fn footer(mut self, e: impl Into<Element<V>>) -> Self {
        self.footer = Some(e.into());
        self
    }
}

impl<V> Component<V> for Card<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let theme = cx.theme();
        let slot = || div().px(24.0);
        let header = self.header.map(|h| {
            let desc = h.description.map(|d| text(d).text_color(theme.muted_foreground).text_size(theme.typography.sm));
            slot()
                .flex_col()
                .gap(6.0)
                .child(div().font_weight(600).child(text(h.title)))
                .children(desc)
        });
        let content = self.content.map(|c| slot().flex_col().child(c));
        let footer = self.footer.map(|f| slot().items_center().child(f));
        div()
            .flex_col()
            .gap(24.0)
            .py(24.0)
            .rounded(theme.radius + 4.0)
            .bg(theme.card)
            .border(1.0, theme.border)
            .shadow(Color::BLACK.alpha(0.05), 3.0)
            .text_color(theme.card_foreground)
            .children(header)
            .children(content)
            .children(footer)
    }
}
