//! Tweens de valores por chave (id de elemento), consultados durante o render.
use super::easing::Transition;
use crate::element::ElementId;
use std::collections::HashMap;
use std::time::Instant;

struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    transition: Transition,
    /// Geração do último render que consultou (descarta tweens órfãos).
    seen: u32,
}

impl Tween {
    fn progress(&self, now: Instant) -> f32 {
        let d = self.transition.duration.as_secs_f32();
        if d <= 0.0 {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f32() / d).min(1.0)
    }

    fn value(&self, now: Instant) -> f32 {
        let t = self.progress(now);
        self.from + (self.to - self.from) * self.transition.easing.apply(t)
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
        let t = self.tweens.entry(key).or_insert(Tween { from, to: target, start: now, transition, seen: g });
        t.seen = g;
        if t.to != target {
            t.from = t.value(now);
            t.to = target;
            t.start = now;
            t.transition = transition;
        }
        let running = t.from != t.to && t.progress(now) < 1.0;
        self.active |= running;
        if running { t.value(now) } else { t.to }
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
