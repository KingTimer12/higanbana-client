//! Progress (kit): trilho na cor da barra a 20% e preenchimento animado.
use super::Component;
use crate::anim::Transition;
use crate::color::Color;
use crate::cx::RenderCx;
use crate::element::{div, Element, ElementId};
use crate::style::Styled;

/// Alturas do kit: 4, 6, 8 e 10 px (raio = metade).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgressSize {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

impl ProgressSize {
    pub fn height(self) -> f32 {
        match self {
            ProgressSize::XSmall => 4.0,
            ProgressSize::Small => 6.0,
            ProgressSize::Medium => 8.0,
            ProgressSize::Large => 10.0,
        }
    }
}

/// Mudanças de valor deslizam (duração "normal" do kit).
const SLIDE: Transition = Transition::ms(240);

/// `Progress::new("dl").value(40.0).size(ProgressSize::XSmall)`; valor em 0..100.
pub struct Progress<V> {
    id: ElementId,
    value: f32,
    size: ProgressSize,
    color: Option<Color>,
    _v: std::marker::PhantomData<fn(&mut V)>,
}

impl<V: 'static> Progress<V> {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Progress { id: id.into(), value: 0.0, size: ProgressSize::default(), color: None, _v: std::marker::PhantomData }
    }

    /// 0..100 (fora disso é preso).
    pub fn value(mut self, v: f32) -> Self {
        self.value = v;
        self
    }

    pub fn size(mut self, s: ProgressSize) -> Self {
        self.size = s;
        self
    }

    /// Cor da barra (padrão: primária).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// Id do preenchimento (testes).
    pub fn fill_id(id: impl Into<ElementId>) -> ElementId {
        let id = id.into();
        ElementId(id.0.rotate_left(7) ^ ElementId::from("preenchimento").0)
    }
}

impl<V: 'static> Component<V> for Progress<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let c = self.color.unwrap_or(cx.theme().primary);
        let target = if self.value.is_finite() { self.value.clamp(0.0, 100.0) } else { 0.0 };
        let v = cx.animate(Self::fill_id(self.id), target, SLIDE);
        let (h, r) = (self.size.height(), self.size.height() / 2.0);
        let fill = div().id(Self::fill_id(self.id)).absolute().top(0.0).left(0.0).h_full().w(crate::pct(v)).rounded(r).bg(c);
        div().id(self.id).relative().w_full().h(h).rounded(r).bg(c.alpha(c.a * 0.2)).child(fill)
    }
}
