//! Tradução de eventos de janela em handlers, mudanças de estado e sujeira.
use super::run::{Dirty, Runner};
use super::View;
use crate::cx::EventCx;
use crate::gfx::Event;
use crate::input::{Key, KeyEvent};

impl<V: View> Runner<'_, V> {
    /// Processa um evento. `false` = encerrar o app.
    pub(super) fn handle(&mut self, ev: Event) -> bool {
        let focus_before = self.tree.focus_info();
        let mut cx = EventCx::default();
        match ev {
            Event::Close => return false,
            Event::Resize { size, scale } => {
                if scale != self.scale {
                    self.scale = scale;
                    self.tree.clear_measure_cache();
                }
                self.viewport = size;
                self.mark(Dirty::Relayout);
            }
            Event::MouseMove { position, mods } => {
                if self.tree.pointer_move(position, mods, &mut self.view, &mut cx) {
                    self.mark(Dirty::Repaint);
                }
            }
            Event::MouseDown { position, button, mods } => {
                self.tree.mouse_down(position, button, mods, &mut self.view, &mut cx);
            }
            Event::MouseUp { position, button, mods } => {
                self.tree.mouse_up(position, button, mods, &mut self.view, &mut cx);
            }
            Event::Scroll { delta, .. } => {
                if self.tree.scroll(delta.y) {
                    self.mark(Dirty::Repaint);
                }
            }
            Event::KeyDown { key, repeat, mods } => {
                let ev = KeyEvent { key: Key::from_vk(key), mods, repeat };
                self.tree.key_down(&ev, &mut self.view, &mut cx);
            }
            Event::Char(c) => {
                self.tree.char_input(c, &mut self.view, &mut cx);
            }
            Event::KeyUp { .. } => {}
            Event::Wake => self.mark(Dirty::Rebuild),
            Event::WindowState(s) => {
                if s != self.window_state {
                    self.window_state = s;
                    self.mark(Dirty::Rebuild);
                }
            }
        }
        if matches!(ev, Event::MouseMove { .. } | Event::MouseDown { .. } | Event::MouseUp { .. }) {
            self.win.set_cursor(self.tree.cursor());
        }
        let alive = self.apply(cx);
        if self.tree.focus_info() != focus_before {
            self.mark(Dirty::Rebuild);
        }
        alive
    }
}
