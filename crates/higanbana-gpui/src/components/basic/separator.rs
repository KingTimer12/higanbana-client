//! Separador de 1px na cor `border`, horizontal ou vertical.
use super::Component;
use crate::cx::RenderCx;
use crate::element::{div, Element};
use crate::style::Styled;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Linha de 1px: ocupa a largura (horizontal) ou estica na altura do pai em linha (vertical).
#[derive(Debug, Clone, Copy, Default)]
pub struct Separator {
    orientation: Orientation,
}

impl Separator {
    pub fn horizontal() -> Self {
        Self { orientation: Orientation::Horizontal }
    }

    pub fn vertical() -> Self {
        Self { orientation: Orientation::Vertical }
    }
}

impl<V> Component<V> for Separator {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let e = div().flex_shrink_0().bg(cx.theme().border);
        match self.orientation {
            Orientation::Horizontal => e.h(1.0).w_full(),
            Orientation::Vertical => e.w(1.0),
        }
    }
}
