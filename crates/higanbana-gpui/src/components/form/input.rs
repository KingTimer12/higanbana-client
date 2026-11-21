//! Input (shadcn): campo de texto de uma linha com placeholder, senha, erro e envio.
use super::input_keys::{attach, Callbacks, TextFn};
use super::password::patch_field;
use super::style::{disabled_look, ring};
use crate::color::Color;
use crate::components::basic::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{text_field, Element, ElementId};
use crate::style::{pct, Length, Styled};
use crate::text_input::TextInputState;
use std::borrow::Cow;
use std::rc::Rc;

/// Campo de texto estilizado. O `TextInputState` mora na view; `lens` diz onde.
///
/// ```ignore
/// Input::new("email", &self.email, |v: &mut App| &mut v.email)
///     .placeholder("voce@exemplo.com")
///     .on_submit(|v, texto, cx| v.enviar(texto))
///     .render(cx)
/// ```
pub struct Input<'a, V, L> {
    id: ElementId,
    state: &'a TextInputState,
    lens: L,
    placeholder: Option<Cow<'static, str>>,
    width: Length,
    disabled: bool,
    password: bool,
    invalid: bool,
    cbs: Callbacks<V>,
}

impl<'a, V: 'static, L> Input<'a, V, L>
where
    L: Fn(&mut V) -> &mut TextInputState + Clone + 'static,
{
    pub fn new(id: impl Into<ElementId>, state: &'a TextInputState, lens: L) -> Self {
        Self {
            id: id.into(),
            state,
            lens,
            placeholder: None,
            width: pct(100.0),
            disabled: false,
            password: false,
            invalid: false,
            cbs: Callbacks { change: None, submit: None },
        }
    }

    /// Texto cinza com o campo vazio (sobrepõe o do estado).
    pub fn placeholder(mut self, text: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Largura (padrão: 100% do pai, como `w-full`).
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        self.width = w.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Mostra bolinhas no lugar do texto e bloqueia copiar/recortar.
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    /// Estado de erro: borda e anel na cor `destructive`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Chamado a cada edição do texto, com o texto novo.
    pub fn on_change(mut self, f: impl Fn(&mut V, &str, &mut EventCx) + 'static) -> Self {
        self.cbs.change = Some(Rc::new(f) as TextFn<V>);
        self
    }

    /// Chamado com Enter, com o texto atual.
    pub fn on_submit(mut self, f: impl Fn(&mut V, &str, &mut EventCx) + 'static) -> Self {
        self.cbs.submit = Some(Rc::new(f) as TextFn<V>);
        self
    }
}

impl<V: 'static, L> Component<V> for Input<'_, V, L>
where
    L: Fn(&mut V) -> &mut TextInputState + Clone + 'static,
{
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = cx.theme();
        let accent = if self.invalid { t.destructive } else { t.ring };
        let border = if self.invalid { t.destructive } else { t.input };
        let bg = if t.dark { t.input.alpha(0.3) } else { Color::TRANSPARENT };
        let mut e = text_field(self.id, self.state, self.lens.clone())
            .w(self.width)
            .h(36.0)
            .px(12.0)
            .py(4.0)
            .rounded(t.radius - 2.0)
            .border(1.0, border)
            .bg(bg)
            .text_size(t.typography.sm)
            .text_color(t.foreground)
            .placeholder_color(t.muted_foreground)
            .selection_color(t.primary.alpha(0.35))
            .caret_color(t.foreground)
            .focus(ring(accent))
            .when(self.invalid, |e| e.shadow(t.destructive.alpha(0.2), 3.0));
        patch_field(&mut e, self.state, self.password, self.placeholder);
        let e = attach(e, self.lens, self.password, self.cbs);
        disabled_look(e, self.disabled)
    }
}
