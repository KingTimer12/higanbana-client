//! Contagem para esconder controles/cursor depois de um tempo sem mexer.
use std::time::{Duration, Instant};

/// `AutoHide::new(2500 ms, agora)`; `poke` em cada movimento/tecla, `tick` no render.
#[derive(Debug, Clone, Copy)]
pub struct AutoHide {
    after: Duration,
    last: Instant,
    idle: bool,
}

impl AutoHide {
    pub fn new(after: Duration, now: Instant) -> Self {
        AutoHide { after, last: now, idle: false }
    }

    /// Atividade: reinicia a contagem. `true` se estava oculto (precisa redesenhar).
    pub fn poke(&mut self, now: Instant) -> bool {
        self.last = now;
        std::mem::take(&mut self.idle)
    }

    /// Quanto falta para ficar ocioso (`None` = já está; marca `idle`).
    pub fn tick(&mut self, now: Instant) -> Option<Duration> {
        let gone = now.saturating_duration_since(self.last);
        if gone >= self.after {
            self.idle = true;
            None
        } else {
            Some(self.after - gone)
        }
    }

    /// Ocioso e nada segurando (pausado, menu aberto, arrastando...).
    pub fn hidden(&self, hold: bool) -> bool {
        self.idle && !hold
    }

    pub fn idle(&self) -> bool {
        self.idle
    }
}
