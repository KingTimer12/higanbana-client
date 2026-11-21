//! Construtores de interação: foco, teclado, ponteiro, hover e desabilitado.
use super::handlers::{Interactivity, PointerEvent};
use super::{Element, ElementId};
use crate::cx::EventCx;
use crate::input::KeyEvent;
use crate::style::Style;

impl<V> Element<V> {
    pub(crate) fn interact(&mut self) -> &mut Interactivity<V> {
        self.interact.get_or_insert_with(Default::default)
    }

    /// Pode receber foco (clique ou Tab). Precisa de `.id(...)` para o foco
    /// sobreviver às reconstruções; prefira `.track_focus(id)`.
    pub fn focusable(mut self) -> Self {
        self.interact().focusable = true;
        self
    }

    /// `.id(id).focusable()`: o foco é seguido por este id.
    pub fn track_focus(self, id: impl Into<ElementId>) -> Self {
        self.id(id).focusable()
    }

    /// Estilo aplicado enquanto o elemento tem foco (sem relayout).
    pub fn focus(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.interact().focus_style = Some(Box::new(f));
        self
    }

    /// Desabilitado: sem clique, foco, hover nem handlers (no elemento e nos filhos).
    pub fn disabled(mut self, disabled: bool) -> Self {
        if disabled || self.interact.is_some() {
            self.interact().disabled = disabled;
        }
        self
    }

    /// Estilo aplicado quando `.disabled(true)` (ex.: opacidade 0.5).
    pub fn disabled_style(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.interact().disabled_style = Some(Box::new(f));
        self
    }

    /// Tecla pressionada com o foco neste elemento ou num descendente.
    /// Chame `cx.stop_propagation()` para não chegar aos ancestrais.
    pub fn on_key_down(mut self, f: impl Fn(&mut V, &KeyEvent, &mut EventCx) + 'static) -> Self {
        self.interact().on_key_down = Some(Box::new(f));
        self
    }

    /// Caractere digitado (já com shift/IME aplicados), mesma propagação das teclas.
    pub fn on_char(mut self, f: impl Fn(&mut V, char, &mut EventCx) + 'static) -> Self {
        self.interact().on_char = Some(Box::new(f));
        self
    }

    /// Botão desceu sobre o elemento. Com `on_mouse_move`, o ponteiro fica capturado
    /// até soltar (arraste continua fora da caixa).
    pub fn on_mouse_down(mut self, f: impl Fn(&mut V, &PointerEvent, &mut EventCx) + 'static) -> Self {
        self.interact().on_mouse_down = Some(Box::new(f));
        self
    }

    /// Botão subiu sobre o elemento (ou em qualquer lugar, se ele capturou o ponteiro).
    pub fn on_mouse_up(mut self, f: impl Fn(&mut V, &PointerEvent, &mut EventCx) + 'static) -> Self {
        self.interact().on_mouse_up = Some(Box::new(f));
        self
    }

    /// Botões laterais do mouse soltos sobre o elemento (voltar/avançar).
    /// Sobe até o primeiro ancestral com handler.
    pub fn on_navigate(mut self, f: impl Fn(&mut V, crate::gfx::NavDirection, &mut EventCx) + 'static) -> Self {
        self.interact().on_navigate = Some(Box::new(f));
        self
    }

    /// Mouse moveu sobre o elemento, ou em qualquer lugar durante a captura.
    pub fn on_mouse_move(mut self, f: impl Fn(&mut V, &PointerEvent, &mut EventCx) + 'static) -> Self {
        self.interact().on_mouse_move = Some(Box::new(f));
        self
    }

    /// Qualquer movimento do mouse dentro do elemento, mesmo sobre filhos com
    /// `on_mouse_move` próprio (ex.: mostrar controles de player).
    pub fn on_mouse_move_passive(mut self, f: impl Fn(&mut V, &PointerEvent, &mut EventCx) + 'static) -> Self {
        self.interact().on_move_passive = Some(Box::new(f));
        self
    }

    /// Mouse entrou (`true`) ou saiu (`false`) do elemento. Use com `.id(...)`.
    pub fn on_hover(mut self, f: impl Fn(&mut V, bool, &mut EventCx) + 'static) -> Self {
        self.interact().on_hover = Some(Box::new(f));
        self
    }
}
