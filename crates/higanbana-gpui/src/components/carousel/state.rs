//! Estado do Carousel. O auto-avanço não usa timer: o item atual é calculado
//! a partir do relógio (`current(now)`), e o render só agenda o próximo frame
//! para o instante da próxima troca. Sem render, nada roda.
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CarouselState {
    len: usize,
    /// Item na base `since` (o atual é este mais os avanços automáticos desde então).
    index: usize,
    looping: bool,
    interval: Option<Duration>,
    since: Instant,
    paused_at: Option<Instant>,
}

impl CarouselState {
    pub fn new(len: usize) -> Self {
        Self { len, index: 0, looping: false, interval: None, since: Instant::now(), paused_at: None }
    }

    /// Do último para o primeiro (e vice-versa).
    pub fn looping(mut self, on: bool) -> Self {
        self.looping = on;
        self
    }

    /// Avança sozinho a cada `every`.
    pub fn auto_advance(mut self, every: Duration) -> Self {
        self.interval = Some(every).filter(|d| !d.is_zero());
        self
    }

    /// Marca o início da contagem do auto-avanço (padrão: criação).
    pub fn started(mut self, at: Instant) -> Self {
        self.since = at;
        self
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn steps(&self, now: Instant) -> usize {
        let (Some(every), true) = (self.interval, self.len > 1) else { return 0 };
        let at = self.paused_at.unwrap_or(now);
        (at.saturating_duration_since(self.since).as_nanos() / every.as_nanos()) as usize
    }

    /// Item mostrado em `now`.
    pub fn current(&self, now: Instant) -> usize {
        if self.len == 0 {
            return 0;
        }
        let i = self.index + self.steps(now);
        if self.looping { i % self.len } else { i.min(self.len - 1) }
    }

    /// Instante da próxima troca automática, se houver.
    pub fn next_change(&self, now: Instant) -> Option<Instant> {
        let every = self.interval?;
        if self.len < 2 || self.paused_at.is_some() || (!self.looping && self.current(now) + 1 >= self.len) {
            return None;
        }
        Some(self.since + every * (self.steps(now) as u32 + 1))
    }

    pub fn has_next(&self, now: Instant) -> bool {
        self.len > 1 && (self.looping || self.current(now) + 1 < self.len)
    }

    pub fn has_prev(&self, now: Instant) -> bool {
        self.len > 1 && (self.looping || self.current(now) > 0)
    }

    /// Vai ao item `i` e recomeça a contagem. `true` se mudou.
    pub fn go(&mut self, i: usize, now: Instant) -> bool {
        let before = self.current(now);
        self.index = i.min(self.len.saturating_sub(1));
        self.since = now;
        if self.paused_at.is_some() {
            self.paused_at = Some(now);
        }
        self.index != before
    }

    pub fn next(&mut self, now: Instant) -> bool {
        if !self.has_next(now) {
            return false;
        }
        let i = (self.current(now) + 1) % self.len;
        self.go(i, now)
    }

    pub fn prev(&mut self, now: Instant) -> bool {
        if !self.has_prev(now) {
            return false;
        }
        let i = (self.current(now) + self.len - 1) % self.len;
        self.go(i, now)
    }

    /// Troca o total de itens (ex.: chegaram mais); o atual fica dentro do intervalo.
    pub fn set_len(&mut self, len: usize, now: Instant) {
        let cur = self.current(now);
        self.len = len;
        let at = self.paused_at.unwrap_or(now);
        let phase = self.interval.map_or(Duration::ZERO, |e| Duration::from_nanos((at.saturating_duration_since(self.since).as_nanos() % e.as_nanos()) as u64));
        self.index = cur.min(len.saturating_sub(1));
        self.since = at - phase;
    }

    /// Pausa (ex.: mouse em cima) e retoma de onde parou.
    pub fn set_paused(&mut self, paused: bool, now: Instant) {
        match (paused, self.paused_at) {
            (true, None) => self.paused_at = Some(now),
            (false, Some(at)) => {
                let cur = self.current(at);
                let phase = self.interval.map_or(Duration::ZERO, |e| Duration::from_nanos((at.saturating_duration_since(self.since).as_nanos() % e.as_nanos()) as u64));
                self.paused_at = None;
                self.index = cur;
                self.since = now - phase;
            }
            _ => {}
        }
    }
}
