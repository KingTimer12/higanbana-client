//! Tweens de valores por chave (id de elemento), consultados durante o render.
use super::easing::Transition;
use super::repeat::{phase, Repeat};
use super::spring::Spring;
use crate::element::ElementId;
use std::collections::HashMap;
use std::time::Instant;

struct Tween {
    from: f32,
    to: f32,
    /// Velocidade inicial (molas: herdada no retarget).
    v0: f32,
    start: Instant,
    transition: Transition,
    /// Geração do último render que consultou (descarta tweens órfãos).
    seen: u32,
}

impl Tween {
    /// Valor, velocidade e se terminou, em `now`.
    fn sample(&self, now: Instant) -> (f32, f32, bool) {
        let tr = &self.transition;
        let t = now.saturating_duration_since(self.start).as_secs_f32();
        if let Some(s) = tr.spring {
            let t = t - tr.delay.as_secs_f32();
            if t <= 0.0 { return (self.from, self.v0, false) }
            let (x, v) = s.sample(self.from, self.to, self.v0, t);
            let done = self.from == self.to && self.v0 == 0.0 || Spring::settled(x, v, self.from, self.to);
            return if done { (self.to, 0.0, true) } else { (x, v, false) };
        }
        if self.from == self.to { return (self.to, 0.0, true) }
        let (p, done) = phase(t, tr.duration.as_secs_f32(), tr.delay.as_secs_f32(), tr.repeat, tr.yoyo);
        if done && tr.repeat == Repeat::Once { return (self.to, 0.0, true) }
        (self.from + (self.to - self.from) * tr.easing.apply(p), 0.0, done)
    }
}

/// Guarda os tweens vivos. O app só agenda frames enquanto algum está ativo.
#[derive(Default)]
pub struct Animator {
    tweens: HashMap<ElementId, Tween>,
    generation: u32,
    active: bool,
}

impl Animator {
    /// Começa um render: zera a marcação de atividade.
    pub fn begin(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active = false;
    }

    /// Termina o render: esquece tweens que ninguém consultou.
    pub fn end(&mut self) {
        let g = self.generation;
        self.tweens.retain(|_, t| t.seen == g);
    }

    /// Algum tween consultado neste render ainda não terminou?
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn len(&self) -> usize {
        self.tweens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tweens.is_empty()
    }

    /// Valor animado de `key` em direção a `target`. Na primeira consulta parte de `from`
    /// (use `from == target` para não animar a entrada). Mudar o alvo reinicia do valor atual.
    pub fn value(&mut self, key: ElementId, from: f32, target: f32, transition: Transition, now: Instant) -> f32 {
        let g = self.generation;
        let t = self.tweens.entry(key).or_insert(Tween { from, to: target, v0: 0.0, start: now, transition, seen: g });
        t.seen = g;
        if t.to != target {
            let (x, v, _) = t.sample(now);
            t.from = x;
            t.v0 = v;
            t.to = target;
            t.start = now;
            t.transition = transition;
        }
        let (x, _, done) = t.sample(now);
        self.active |= !done;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn key() -> ElementId {
        ElementId::from("fade")
    }

    #[test]
    fn active_only_while_running() {
        let t0 = Instant::now();
        let tr = Transition::ms(100).easing(super::super::Easing::Linear);
        let mut a = Animator::default();
        a.begin();
        assert_eq!(a.value(key(), 0.0, 1.0, tr, t0), 0.0);
        assert!(a.is_active());
        a.end();
        a.begin();
        let mid = a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_millis(50));
        assert!((mid - 0.5).abs() < 1e-3);
        a.end();
        a.begin();
        assert_eq!(a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_millis(150)), 1.0);
        assert!(!a.is_active(), "terminou: nada de frames extras");
    }

    #[test]
    fn forever_stops_when_not_queried() {
        let t0 = Instant::now();
        let tr = Transition::ms(100).repeat(super::super::Repeat::Forever);
        let mut a = Animator::default();
        a.begin();
        a.value(key(), 0.0, 1.0, tr, t0);
        a.end();
        a.begin();
        a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_secs(5));
        assert!(a.is_active(), "infinito segue ativo enquanto consultado");
        a.end();
        a.begin();
        a.end();
        assert!(a.is_empty() && !a.is_active());
    }

    #[test]
    fn delay_then_yoyo() {
        let t0 = Instant::now();
        let tr = Transition::ms(100).easing(super::super::Easing::Linear).delay(50).repeat(super::super::Repeat::Times(2)).yoyo();
        let mut a = Animator::default();
        a.begin();
        assert_eq!(a.value(key(), 0.0, 1.0, tr, t0), 0.0);
        assert_eq!(a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_millis(40)), 0.0);
        assert!((a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_millis(200)) - 0.5).abs() < 1e-3);
        assert_eq!(a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_millis(400)), 0.0);
    }

    #[test]
    fn retarget_keeps_continuity() {
        let t0 = Instant::now();
        let tr = Transition::spring(170.0, 26.0);
        let mut a = Animator::default();
        a.begin();
        a.value(key(), 0.0, 1.0, tr, t0);
        let t1 = t0 + Duration::from_millis(100);
        let before = a.value(key(), 0.0, 1.0, tr, t1);
        let after = a.value(key(), 0.0, 0.0, tr, t1);
        assert!(before > 0.1 && before < 1.0);
        assert!((before - after).abs() < 1e-3, "{before} vs {after}");
        let later = a.value(key(), 0.0, 0.0, tr, t1 + Duration::from_millis(16));
        assert!(later > before - 0.05, "velocidade preservada: não despenca ({later})");
    }

    #[test]
    fn spring_active_until_settled() {
        let t0 = Instant::now();
        let tr = Transition::spring(170.0, 26.0);
        let mut a = Animator::default();
        a.begin();
        a.value(key(), 0.0, 1.0, tr, t0);
        a.end();
        a.begin();
        a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_millis(100));
        assert!(a.is_active());
        a.end();
        a.begin();
        assert_eq!(a.value(key(), 0.0, 1.0, tr, t0 + Duration::from_secs(3)), 1.0);
        assert!(!a.is_active());
    }

    #[test]
    fn retarget_starts_from_current_and_gc() {
        let t0 = Instant::now();
        let tr = Transition::ms(100).easing(super::super::Easing::Linear);
        let mut a = Animator::default();
        a.begin();
        assert_eq!(a.value(key(), 1.0, 1.0, tr, t0), 1.0, "from == target: entra parado");
        assert!(!a.is_active());
        let v = a.value(key(), 1.0, 0.0, tr, t0 + Duration::from_millis(10));
        assert_eq!(v, 1.0);
        assert!(a.is_active());
        a.end();
        a.begin();
        a.end();
        assert!(a.is_empty(), "não consultado no render = descartado");
    }
}
