//! Decorations da janela: barra nativa ou desenhada pelo app, regiões de hit test,
//! comandos (min/max/fechar/fullscreen) e estado.
use crate::geometry::Rect;
use higanbana_gpui_sys as sys;

/// Barra de título nativa do sistema ou desenhada pelo app (`TitleBar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Decorations {
    #[default]
    Native,
    /// Sem barra nativa: o app marca a área de arrastar e os botões
    /// (`.window_drag()`, `.window_control(..)`). Snap, sombra e animações continuam.
    Custom,
}

/// O que uma região da janela faz para o sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitKind {
    /// Conteúdo normal (botões dentro da barra, overlays).
    Client,
    /// Arrasta a janela; duplo clique maximiza.
    Caption,
    /// Botão maximizar (snap layouts do Windows 11).
    Maximize,
}

/// Região em pixels lógicos. A última que contém o ponto vence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRegion {
    pub rect: Rect,
    pub kind: HitKind,
}

/// Comando de janela pedido pelo app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    Minimize,
    Maximize,
    Restore,
    ToggleMaximize,
    Close,
    EnterFullscreen,
    ExitFullscreen,
    ToggleFullscreen,
}

/// Estado atual da janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowState {
    pub maximized: bool,
    pub minimized: bool,
    pub fullscreen: bool,
    pub focused: bool,
}

impl WindowState {
    pub(crate) fn from_flags(f: u32) -> Self {
        WindowState {
            maximized: f & sys::HG_WINDOW_MAXIMIZED != 0,
            minimized: f & sys::HG_WINDOW_MINIMIZED != 0,
            fullscreen: f & sys::HG_WINDOW_FULLSCREEN != 0,
            focused: f & sys::HG_WINDOW_FOCUSED != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_from_flags() {
        let s = WindowState::from_flags(sys::HG_WINDOW_MAXIMIZED | sys::HG_WINDOW_FOCUSED);
        assert_eq!(s, WindowState { maximized: true, focused: true, ..Default::default() });
    }
}
