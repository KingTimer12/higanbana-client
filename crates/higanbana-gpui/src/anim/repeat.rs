//! Tempo decorrido → progresso de uma transição com atraso, repetições e ida-e-volta.

/// Quantas vezes a transição roda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Once,
    Times(u32),
    /// Para só quando ninguém mais consulta o tween (elemento saiu da árvore).
    Forever,
}

/// Progresso 0..1 (antes da curva) e se acabou. Em `yoyo`, ciclos ímpares voltam de 1 a 0;
/// cada ida ou volta conta como um ciclo.
pub fn phase(elapsed: f32, duration: f32, delay: f32, repeat: Repeat, yoyo: bool) -> (f32, bool) {
    let t = elapsed - delay;
    if t <= 0.0 {
        return (0.0, false);
    }
    if duration <= 0.0 {
        return (1.0, repeat != Repeat::Forever);
    }
    let cycles = match repeat {
        Repeat::Once => Some(1),
        Repeat::Times(n) => Some(n.max(1)),
        Repeat::Forever => None,
    };
    let k = (t / duration).floor();
    if let Some(n) = cycles.filter(|&n| k >= n as f32) {
        let back = yoyo && n % 2 == 0;
        return (if back { 0.0 } else { 1.0 }, true);
    }
    let p = t / duration - k;
    let back = yoyo && (k as u64) % 2 == 1;
    (if back { 1.0 - p } else { p }, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn delay_holds_start() {
        assert_eq!(phase(0.03, 0.1, 0.05, Repeat::Once, false), (0.0, false));
    }

    #[test]
    fn once_finishes() {
        let (p, done) = phase(0.1, 0.1, 0.0, Repeat::Once, false);
        assert!(close(p, 1.0) && done);
    }

    #[test]
    fn times_two_restarts() {
        let (p, done) = phase(0.15, 0.1, 0.0, Repeat::Times(2), false);
        assert!(close(p, 0.5) && !done);
        let (p, done) = phase(0.25, 0.1, 0.0, Repeat::Times(2), false);
        assert!(close(p, 1.0) && done);
    }

    #[test]
    fn yoyo_returns() {
        let (p, _) = phase(0.15, 0.1, 0.0, Repeat::Times(2), true);
        assert!(close(p, 0.5));
        let (p, done) = phase(0.18, 0.1, 0.0, Repeat::Times(2), true);
        assert!(close(p, 0.2) && !done);
        let (p, done) = phase(0.3, 0.1, 0.0, Repeat::Times(2), true);
        assert!(close(p, 0.0) && done, "yoyo par termina no início");
    }

    #[test]
    fn forever_never_done() {
        let (_, done) = phase(10.0, 0.1, 0.0, Repeat::Forever, false);
        assert!(!done);
    }
}
