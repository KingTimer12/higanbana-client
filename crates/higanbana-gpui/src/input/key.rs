//! Teclas nomeadas (a partir do virtual key do Windows) e o evento de tecla.
use crate::gfx::Modifiers;

/// Tecla pressionada. Letras e dígitos vêm em minúsculas (`Key::Char('a')`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Tab,
    Enter,
    Escape,
    Backspace,
    Delete,
    Space,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1..F24.
    F(u8),
    /// `a`..`z`, `0`..`9`.
    Char(char),
    /// Qualquer outra (virtual key cru).
    Other(u32),
}

impl Key {
    /// Converte o virtual key do Windows.
    pub fn from_vk(vk: u32) -> Key {
        match vk {
            0x09 => Key::Tab,
            0x0D => Key::Enter,
            0x1B => Key::Escape,
            0x08 => Key::Backspace,
            0x2E => Key::Delete,
            0x20 => Key::Space,
            0x25 => Key::Left,
            0x26 => Key::Up,
            0x27 => Key::Right,
            0x28 => Key::Down,
            0x24 => Key::Home,
            0x23 => Key::End,
            0x21 => Key::PageUp,
            0x22 => Key::PageDown,
            0x70..=0x87 => Key::F((vk - 0x6F) as u8),
            0x30..=0x39 => Key::Char((b'0' + (vk - 0x30) as u8) as char),
            0x41..=0x5A => Key::Char((b'a' + (vk - 0x41) as u8) as char),
            _ => Key::Other(vk),
        }
    }
}

/// Tecla entregue a `on_key_down`: borbulha do elemento focado até a raiz
/// enquanto ninguém chamar `cx.stop_propagation()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Modifiers,
    /// Repetição automática (tecla segurada).
    pub repeat: bool,
}

impl KeyEvent {
    pub fn new(key: Key, mods: Modifiers) -> Self {
        Self { key, mods, repeat: false }
    }

    /// Atalho `Ctrl+<c>` sem Alt (ex.: `ev.ctrl('c')`).
    pub fn ctrl(&self, c: char) -> bool {
        self.mods.ctrl && !self.mods.alt && self.key == Key::Char(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vk_mapping() {
        assert_eq!(Key::from_vk(0x41), Key::Char('a'));
        assert_eq!(Key::from_vk(0x39), Key::Char('9'));
        assert_eq!(Key::from_vk(0x70), Key::F(1));
        assert_eq!(Key::from_vk(0x1B), Key::Escape);
        assert_eq!(Key::from_vk(0xFF), Key::Other(0xFF));
    }
}
