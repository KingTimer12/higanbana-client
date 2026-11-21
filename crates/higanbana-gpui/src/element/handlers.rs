//! Eventos e handlers tipados na view; extras de interação guardados numa caixa só
//! (elementos sem interação não pagam memória por eles).
use crate::cx::EventCx;
use crate::geometry::{Point, Rect};
use crate::gfx::{Modifiers, MouseButton};
use crate::input::KeyEvent;
use crate::style::Style;

/// Clique entregue aos handlers; posição em pixels lógicos da janela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseEvent {
    pub position: Point,
    pub button: MouseButton,
    pub mods: Modifiers,
}

/// Evento de ponteiro (desce/sobe/move) com posição relativa ao elemento.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    /// Pixels lógicos da janela.
    pub position: Point,
    /// Relativa ao canto superior esquerdo de `bounds` (pode sair dele durante arraste).
    pub local: Point,
    /// Caixa do elemento dono do handler.
    pub bounds: Rect,
    pub button: MouseButton,
    pub mods: Modifiers,
    /// Botão esquerdo segurado (arraste com captura do ponteiro).
    pub pressed: bool,
    /// Em campos de texto: índice de byte sob o ponteiro.
    pub text_index: Option<usize>,
}

impl PointerEvent {
    /// Fração horizontal 0..1 dentro de `bounds` (útil para sliders).
    pub fn fraction_x(&self) -> f32 {
        if self.bounds.size.width <= 0.0 { 0.0 } else { (self.local.x / self.bounds.size.width).clamp(0.0, 1.0) }
    }
}

pub type ClickHandler<V> = Box<dyn Fn(&mut V, &MouseEvent, &mut EventCx)>;
pub type KeyHandler<V> = Box<dyn Fn(&mut V, &KeyEvent, &mut EventCx)>;
pub type CharHandler<V> = Box<dyn Fn(&mut V, char, &mut EventCx)>;
pub type PointerHandler<V> = Box<dyn Fn(&mut V, &PointerEvent, &mut EventCx)>;
/// Chamado uma vez quando o elemento entra na área visível (com margem).
pub type VisibleHandler<V> = Box<dyn FnOnce(&mut V, &mut EventCx)>;
pub type HoverHandler<V> = Box<dyn Fn(&mut V, bool, &mut EventCx)>;
pub type NavHandler<V> = Box<dyn Fn(&mut V, crate::gfx::NavDirection, &mut EventCx)>;
pub type DismissHandler<V> = Box<dyn Fn(&mut V, &mut EventCx)>;
/// Transforma o estilo base num estilo de estado (hover, foco, desabilitado).
pub type StyleFn = Box<dyn Fn(Style) -> Style>;
/// Nome antigo, mantido.
pub type HoverStyle = StyleFn;

/// Foco, teclado, ponteiro e estados. Só existe em elementos que usam algum.
pub(crate) struct Interactivity<V> {
    pub focusable: bool,
    pub focus_style: Option<StyleFn>,
    pub disabled: bool,
    pub disabled_style: Option<StyleFn>,
    pub on_key_down: Option<KeyHandler<V>>,
    pub on_char: Option<CharHandler<V>>,
    pub on_mouse_down: Option<PointerHandler<V>>,
    pub on_mouse_up: Option<PointerHandler<V>>,
    pub on_mouse_move: Option<PointerHandler<V>>,
    /// Qualquer movimento dentro do elemento (sobe pelos pais; não consome).
    pub on_move_passive: Option<PointerHandler<V>>,
    pub on_hover: Option<HoverHandler<V>>,
    pub on_navigate: Option<NavHandler<V>>,
    pub on_visible: Option<(f32, VisibleHandler<V>)>,
}

impl<V> Default for Interactivity<V> {
    fn default() -> Self {
        Self {
            focusable: false,
            focus_style: None,
            disabled: false,
            disabled_style: None,
            on_key_down: None,
            on_char: None,
            on_mouse_down: None,
            on_mouse_up: None,
            on_mouse_move: None,
            on_move_passive: None,
            on_hover: None,
            on_navigate: None,
            on_visible: None,
        }
    }
}

impl<V> Interactivity<V> {
    /// Tem algum handler de ponteiro (desce/sobe/move)?
    pub fn has_pointer(&self) -> bool {
        self.on_mouse_down.is_some() || self.on_mouse_up.is_some() || self.on_mouse_move.is_some()
    }
}
