//! Teclado padrão de um campo de texto (setas, Home/End, apagar, Ctrl+A/C/X/V).
use super::edit::Clipboard;
use super::state::TextInputState;
use crate::input::{Key, KeyEvent};

/// Resultado de uma tecla/caractere no campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputOutcome {
    /// Não é com o campo: deixa borbulhar (Tab, Enter, Escape...).
    Ignored,
    /// Tratado sem mudar o texto (cursor, seleção, copiar).
    Moved,
    /// O texto mudou.
    Edited,
}

impl InputOutcome {
    pub fn handled(self) -> bool {
        self != InputOutcome::Ignored
    }
}

fn edited(changed: bool) -> InputOutcome {
    if changed { InputOutcome::Edited } else { InputOutcome::Moved }
}

impl TextInputState {
    /// Aplica uma tecla. Atalhos com Ctrl usam `clip` para copiar/colar.
    pub fn handle_key(&mut self, ev: &KeyEvent, clip: &mut dyn Clipboard) -> InputOutcome {
        let (word, shift) = (ev.mods.ctrl, ev.mods.shift);
        match ev.key {
            Key::Left => self.move_left(word, shift),
            Key::Right => self.move_right(word, shift),
            Key::Home | Key::Up => self.move_home(shift),
            Key::End | Key::Down => self.move_end(shift),
            Key::Backspace => return edited(self.backspace(word)),
            Key::Delete => return edited(self.delete(word)),
            _ if ev.ctrl('a') => self.select_all(),
            _ if ev.ctrl('c') => {
                self.copy(clip);
            }
            _ if ev.ctrl('x') => return edited(self.cut(clip)),
            _ if ev.ctrl('v') => return edited(self.paste(clip)),
            _ => return InputOutcome::Ignored,
        }
        InputOutcome::Moved
    }

    /// Caractere digitado (evento CHAR). Controles e Ctrl+letra são ignorados.
    pub fn handle_char(&mut self, c: char) -> InputOutcome {
        if c.is_control() {
            return InputOutcome::Ignored;
        }
        let mut buf = [0u8; 4];
        self.insert(c.encode_utf8(&mut buf));
        InputOutcome::Edited
    }
}
