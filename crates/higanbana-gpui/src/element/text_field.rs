//! Elemento campo de texto: pinta texto, seleção e cursor piscando, com rolagem
//! horizontal; o teclado/mouse editam o `TextInputState` da view por uma lente.
use super::{element, Element, ElementId, Kind};
use crate::color::{rgba, Color};
use crate::gfx::CursorStyle;
use crate::style::{Overflow, Styled};
use crate::text_input::{SystemClipboard, TextInputState};
use std::borrow::Cow;
use std::time::Instant;

/// Meio período do piscar do cursor (padrão do Windows).
pub const CARET_BLINK_MS: u64 = 530;

/// Cópia do estado para pintar (a view continua dona do original).
pub(crate) struct FieldData {
    pub text: String,
    pub caret: usize,
    pub anchor: usize,
    pub placeholder: Cow<'static, str>,
    pub epoch: Instant,
    pub caret_color: Option<Color>,
    pub selection_color: Color,
    pub placeholder_color: Option<Color>,
}

/// Campo de texto de uma linha ligado a `state` (lido agora) e a `lens`
/// (como achar o estado na view para editar). Já é focável pelo `id`, usa cursor
/// de texto e recorta o conteúdo; estilize a caixa como qualquer `div`.
///
/// ```ignore
/// text_field("nome", &self.nome, |v: &mut Form| &mut v.nome).w(240.0).px(8.0).py(6.0)
/// ```
pub fn text_field<V: 'static>(
    id: impl Into<ElementId>,
    state: &TextInputState,
    lens: impl Fn(&mut V) -> &mut TextInputState + Clone + 'static,
) -> Element<V> {
    let data = FieldData {
        text: state.text.clone(),
        caret: state.caret,
        anchor: state.anchor,
        placeholder: state.placeholder.clone(),
        epoch: state.epoch,
        caret_color: None,
        selection_color: rgba(0x3b82_f666),
        placeholder_color: None,
    };
    let mut e = element(Kind::TextField(Box::new(data)), Default::default());
    e.style.overflow = Overflow::Hidden;
    let (l1, l2, l3, l4) = (lens.clone(), lens.clone(), lens.clone(), lens);
    e.track_focus(id)
        .cursor(CursorStyle::Text)
        .on_key_down(move |v, ev, cx| {
            if l1(v).handle_key(ev, &mut SystemClipboard).handled() {
                cx.stop_propagation();
                cx.notify();
            }
        })
        .on_char(move |v, c, cx| {
            if l2(v).handle_char(c).handled() {
                cx.stop_propagation();
                cx.notify();
            }
        })
        .on_mouse_down(move |v, ev, cx| {
            if let Some(i) = ev.text_index {
                l3(v).set_caret(i, ev.mods.shift);
                cx.notify();
            }
        })
        .on_mouse_move(move |v, ev, cx| {
            let Some(i) = ev.text_index.filter(|_| ev.pressed) else { return };
            let s = l4(v);
            if s.caret() != i {
                s.set_caret(i, true);
                cx.notify();
            }
        })
}

impl<V> Element<V> {
    fn field(&mut self) -> Option<&mut FieldData> {
        match &mut self.kind {
            Kind::TextField(f) => Some(f),
            _ => None,
        }
    }

    /// Cor do cursor de um `text_field` (padrão: cor do texto).
    pub fn caret_color(mut self, c: Color) -> Self {
        if let Some(f) = self.field() {
            f.caret_color = Some(c);
        }
        self
    }

    /// Cor de fundo da seleção de um `text_field`.
    pub fn selection_color(mut self, c: Color) -> Self {
        if let Some(f) = self.field() {
            f.selection_color = c;
        }
        self
    }

    /// Cor do placeholder de um `text_field` (padrão: texto a 50%).
    pub fn placeholder_color(mut self, c: Color) -> Self {
        if let Some(f) = self.field() {
            f.placeholder_color = Some(c);
        }
        self
    }
}
