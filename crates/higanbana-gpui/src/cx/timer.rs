//! Timers de uma vez: agenda com prazo, sem threads (o laço acorda no prazo).
use super::event::EventCx;
use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Identificador de timer (para `cancel_timer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

impl TimerId {
    pub(crate) fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        TimerId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Callback com a view apagada (a `EventCx` não é genérica).
pub(crate) type TimerFn = Box<dyn FnOnce(&mut dyn Any, &mut EventCx)>;

/// Fila de timers pendentes. Poucos por vez: `Vec` simples, sem heap binário.
#[derive(Default)]
pub struct Timers {
    pending: Vec<(Instant, TimerId, TimerFn)>,
}

impl Timers {
    /// Recebe os pedidos de um handler.
    pub(crate) fn absorb(&mut self, cx: &mut EventCx, now: Instant) {
        for (delay, id, f) in cx.timers.drain(..) {
            self.pending.push((now + delay, id, f));
        }
        for id in cx.cancelled.drain(..) {
            self.pending.retain(|(_, t, _)| *t != id);
        }
    }

    /// Prazo mais próximo, se houver timer.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.pending.iter().map(|(t, _, _)| *t).min()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Retira e roda os timers vencidos, na ordem do prazo. Devolve o contexto acumulado.
    pub(crate) fn fire_due(&mut self, now: Instant, view: &mut dyn Any) -> Option<EventCx> {
        if !self.pending.iter().any(|(t, _, _)| *t <= now) {
            return None;
        }
        let mut due: Vec<_> = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                due.push(self.pending.swap_remove(i));
            } else {
                i += 1;
            }
        }
        due.sort_by_key(|(t, _, _)| *t);
        let mut cx = EventCx::default();
        for (_, _, f) in due {
            f(view, &mut cx);
        }
        // Timers criados pelos próprios timers entram na fila.
        self.absorb(&mut cx, now);
        Some(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[derive(Default)]
    struct V(Vec<u32>);

    #[test]
    fn fires_in_order_and_cancels() {
        let t0 = Instant::now();
        let mut timers = Timers::default();
        let mut cx = EventCx::default();
        cx.spawn_timer(Duration::from_millis(20), |v: &mut V, _| v.0.push(2));
        cx.spawn_timer(Duration::from_millis(10), |v: &mut V, cx| {
            v.0.push(1);
            cx.notify();
        });
        let gone = cx.spawn_timer(Duration::from_millis(5), |v: &mut V, _| v.0.push(99));
        cx.cancel_timer(gone);
        timers.absorb(&mut cx, t0);
        assert_eq!(timers.len(), 2);
        assert_eq!(timers.next_deadline(), Some(t0 + Duration::from_millis(10)));
        let mut v = V::default();
        assert!(timers.fire_due(t0 + Duration::from_millis(5), &mut v).is_none(), "nada vencido");
        let cx = timers.fire_due(t0 + Duration::from_millis(30), &mut v).unwrap();
        assert_eq!(v.0, vec![1, 2]);
        assert!(cx.is_notified());
        assert!(timers.next_deadline().is_none(), "ocioso: sem prazo, laço volta a bloquear");
    }
}
