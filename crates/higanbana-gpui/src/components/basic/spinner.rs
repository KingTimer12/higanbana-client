//! Spinner (kit): ícone `loader` girando uma volta a cada 0,8 s (ease-in-out).
//! Só agenda frames enquanto está na árvore.
use super::Component;
use crate::anim::{Easing, Transition};
use crate::color::Color;
use crate::cx::RenderCx;
use crate::element::{Element, ElementId};
use crate::icon::icon;
use crate::style::Styled;

const TURN: Transition = Transition::ms(800).easing(Easing::EaseInOut);

/// `Spinner::new("carregando").size(24.0).color(c)`; 16 px por padrão (kit: 12/14/16/24).
pub struct Spinner<V> {
    id: ElementId,
    size: f32,
    color: Option<Color>,
    icon: &'static str,
    _v: std::marker::PhantomData<fn(&mut V)>,
}

impl<V: 'static> Spinner<V> {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Spinner { id: id.into(), size: 16.0, color: None, icon: "loader", _v: std::marker::PhantomData }
    }

    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// Outro ícone (ex.: `"loader-circle"`).
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = name;
        self
    }
}

impl<V: 'static> Component<V> for Spinner<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let turn = cx.animate_loop(self.id, TURN);
        let e = icon(self.icon).size(self.size).rotate(turn);
        match self.color {
            Some(c) => e.text_color(c),
            None => e,
        }
    }
}
