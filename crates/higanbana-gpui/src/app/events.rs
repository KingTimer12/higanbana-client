//! Tradução de eventos de janela em mudanças de estado e sujeira.
use super::run::{Dirty, Runner};
use super::View;
use crate::cx::EventCx;
use crate::gfx::Event;

impl<V: View> Runner<'_, V> {
    /// Processa um evento. `false` = encerrar o app.
    pub(super) fn handle(&mut self, ev: Event) -> bool {
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
            Event::MouseMove { position, .. } => {
                if self.tree.mouse_move(position) {
                    self.mark(Dirty::Repaint);
                }
            }
            Event::MouseDown { position, button, .. } => self.tree.mouse_down(position, button),
            Event::MouseUp { position, button, mods } => {
                let mut cx = EventCx::default();
                self.tree.mouse_up(position, button, mods, &mut self.view, &mut cx);
                return self.apply(cx);
            }
            Event::Scroll { delta, .. } => {
                if self.tree.scroll(delta.y) {
                    self.mark(Dirty::Repaint);
                }
            }
            Event::KeyDown { .. } | Event::KeyUp { .. } | Event::Char(_) => {}
        }
        true
    }

    /// Executa os pedidos feitos pelo handler.
    fn apply(&mut self, cx: EventCx) -> bool {
        if cx.quit {
            return false;
        }
        if let Some(theme) = cx.theme {
            self.theme = theme;
        }
        if let Some(kind) = cx.switch_to {
            if let Err(e) = self.ctx.switch_backend(kind) {
                eprintln!("higanbana-gpui: troca para {} falhou: {e}", kind.label());
            }
        }
        if cx.notified {
            self.mark(Dirty::Rebuild);
        }
        true
    }
}
