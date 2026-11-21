//! Eventos de janela tipados, traduzidos de `hg_event`.
use crate::geometry::{point, size, Point, Size};
use higanbana_gpui_sys as sys;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    /// Lateral de trás (XBUTTON1).
    Back,
    /// Lateral da frente (XBUTTON2).
    Forward,
}

/// Sentido dos botões laterais do mouse (`Element::on_navigate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Back,
    Forward,
}

/// Modificadores ativos no momento do evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Evento de janela. Posições em pixels lógicos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Close,
    /// Tamanho lógico novo e escala de DPI.
    Resize { size: Size, scale: f32 },
    MouseMove { position: Point, mods: Modifiers },
    MouseDown { position: Point, button: MouseButton, mods: Modifiers },
    MouseUp { position: Point, button: MouseButton, mods: Modifiers },
    /// Delta em "entalhes" da roda (1.0 = um clique).
    Scroll { delta: Point, mods: Modifiers },
    /// Maximizada/minimizada/fullscreen/foco mudou.
    WindowState(super::decor::WindowState),
    /// Outra thread chamou `Waker::wake`.
    Wake,
    /// `key` é o virtual key do Windows.
    KeyDown { key: u32, repeat: bool, mods: Modifiers },
    KeyUp { key: u32, mods: Modifiers },
    Char(char),
}

impl Modifiers {
    fn from_raw(m: u32) -> Self {
        Self { shift: m & sys::HG_MOD_SHIFT != 0, ctrl: m & sys::HG_MOD_CTRL != 0, alt: m & sys::HG_MOD_ALT != 0 }
    }
}

fn button(b: u32) -> MouseButton {
    match b {
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        _ => MouseButton::Left,
    }
}

impl Event {
    /// O core já entrega o mouse em pixels lógicos; o resize vem em físicos.
    pub(crate) fn from_raw(ev: &sys::hg_event) -> Option<Event> {
        let mods = Modifiers::from_raw(ev.mods);
        let u = &ev.u;
        Some(unsafe {
            match ev.kind {
                sys::hg_event_kind_HG_EVENT_CLOSE => Event::Close,
                sys::hg_event_kind_HG_EVENT_RESIZE => {
                    let s = if u.resize.dpi_scale > 0.0 { u.resize.dpi_scale } else { 1.0 };
                    Event::Resize { size: size(u.resize.width as f32 / s, u.resize.height as f32 / s), scale: s }
                }
                sys::hg_event_kind_HG_EVENT_MOUSE_MOVE => {
                    Event::MouseMove { position: point(u.mouse.x, u.mouse.y), mods }
                }
                sys::hg_event_kind_HG_EVENT_MOUSE_DOWN => {
                    Event::MouseDown { position: point(u.mouse.x, u.mouse.y), button: button(u.mouse.button), mods }
                }
                sys::hg_event_kind_HG_EVENT_MOUSE_UP => {
                    Event::MouseUp { position: point(u.mouse.x, u.mouse.y), button: button(u.mouse.button), mods }
                }
                sys::hg_event_kind_HG_EVENT_SCROLL => Event::Scroll { delta: point(u.scroll.dx, u.scroll.dy), mods },
                sys::hg_event_kind_HG_EVENT_KEY_DOWN => {
                    Event::KeyDown { key: u.key.key, repeat: u.key.repeat != 0, mods }
                }
                sys::hg_event_kind_HG_EVENT_KEY_UP => Event::KeyUp { key: u.key.key, mods },
                sys::hg_event_kind_HG_EVENT_WAKE => Event::Wake,
                sys::hg_event_kind_HG_EVENT_WINDOW_STATE => Event::WindowState(super::decor::WindowState::from_flags(u.state.flags)),
                sys::hg_event_kind_HG_EVENT_CHAR => Event::Char(char::from_u32(u.text.codepoint)?),
                _ => return None,
            }
        })
    }
}
