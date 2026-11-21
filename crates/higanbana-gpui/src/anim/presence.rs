//! Presença (entrada/saída): mantém algo montado até a animação de saída acabar.
use super::animator::Animator;
use super::easing::Transition;
use crate::element::ElementId;
use std::time::Instant;

impl Animator {
    /// Presença 0..1 de algo que aparece/some. Entra partindo de 0 e sai animando até 0.
    /// `None` = invisível e saída concluída: não renderizar. Reabrir no meio da saída
    /// continua do valor atual.
    pub fn presence(&mut self, key: ElementId, visible: bool, transition: Transition, now: Instant) -> Option<f32> {
        let p = self.value(key, 0.0, if visible { 1.0 } else { 0.0 }, transition, now);
        (visible || p > 0.0).then_some(p)
    }
}

#[cfg(test)]
mod tests {
    use crate::anim::{Animator, Easing, Transition};
    use crate::element::ElementId;
    use std::time::{Duration, Instant};

    const LINEAR: Transition = Transition::ms(100).easing(Easing::Linear);

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    #[test]
    fn presence_none_after_exit() {
        let (t0, k) = (Instant::now(), ElementId::from("p"));
        let mut a = Animator::default();
        a.begin();
        assert_eq!(a.presence(k, false, LINEAR, t0), None, "nunca visível: nada");
        assert_eq!(a.presence(k, true, LINEAR, t0), Some(0.0));
        assert_eq!(a.presence(k, true, LINEAR, t0 + ms(150)), Some(1.0));
        let leaving = a.presence(k, false, LINEAR, t0 + ms(200)).unwrap();
        assert_eq!(leaving, 1.0);
        let mid = a.presence(k, false, LINEAR, t0 + ms(250)).unwrap();
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(a.presence(k, false, LINEAR, t0 + ms(400)), None);
    }

    #[test]
    fn presence_reopen_is_continuous() {
        let (t0, k) = (Instant::now(), ElementId::from("p"));
        let mut a = Animator::default();
        a.begin();
        a.presence(k, true, LINEAR, t0);
        a.presence(k, true, LINEAR, t0 + ms(100));
        a.presence(k, false, LINEAR, t0 + ms(100));
        let before = a.presence(k, false, LINEAR, t0 + ms(175)).unwrap();
        let after = a.presence(k, true, LINEAR, t0 + ms(175)).unwrap();
        assert!((before - 0.25).abs() < 1e-3);
        assert!((before - after).abs() < 1e-3, "sem voltar a 0");
    }
}
