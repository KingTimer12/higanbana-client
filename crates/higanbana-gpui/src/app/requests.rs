//! Pedidos dos handlers (foco, timers, frames) e o relógio do laço.
use super::run::{Dirty, Runner};
use super::View;
use crate::cx::EventCx;
use std::time::{Duration, Instant};

/// Intervalo de frame quando não há vsync para frear o laço.
const FRAME: Duration = Duration::from_millis(16);

impl<V: View> Runner<'_, V> {
    /// Executa os pedidos feitos pelos handlers. `false` = encerrar.
    pub(super) fn apply(&mut self, mut cx: EventCx) -> bool {
        if cx.quit {
            return false;
        }
        let now = Instant::now();
        if let Some(theme) = cx.theme.take() {
            self.theme = theme;
        }
        if let Some(kind) = cx.switch_to {
            if let Err(e) = self.ctx.switch_backend(kind) {
                eprintln!("higanbana-gpui: troca para {} falhou: {e}", kind.label());
            }
        }
        if let Some(req) = cx.focus {
            self.tree.apply_focus(req);
        }
        self.timers.absorb(&mut cx, now);
        if cx.animation_frame {
            self.request_frame(now);
        }
        if cx.notified {
            self.mark(Dirty::Rebuild);
        }
        true
    }

    /// Reconstrói no próximo frame (com vsync o `present` já freia o ritmo).
    pub(super) fn request_frame(&mut self, now: Instant) {
        let at = if self.vsync { now } else { now + FRAME };
        self.wake = Some(self.wake.map_or(at, |w| w.min(at)));
    }

    /// Próximo instante em que o laço precisa acordar sem evento (`None` = dormir).
    pub(super) fn deadline(&self) -> Option<Instant> {
        [self.timers.next_deadline(), self.wake, self.blink].into_iter().flatten().min()
    }

    /// Roda o que venceu: timers, reconstrução pedida e piscar do cursor. `false` = encerrar.
    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if let Some(cx) = self.timers.fire_due(now, &mut self.view) {
            if !self.apply(cx) {
                return false;
            }
        }
        if self.wake.is_some_and(|w| w <= now) {
            self.wake = None;
            self.mark(Dirty::Rebuild);
        }
        if self.blink.is_some_and(|b| b <= now) {
            self.blink = None;
            self.mark(Dirty::Repaint);
        }
        true
    }
}
