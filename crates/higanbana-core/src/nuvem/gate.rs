//! Limitador global das requisições à nuvem (listagem e busca). Uma por vez,
//! com intervalo desde a anterior; a interface passa na frente da indexação em
//! segundo plano. Em HTTP 429 todos esperam o mesmo recuo exponencial (5 s,
//! 10 s… até 60 s) e a requisição é repetida. Nenhuma trava fica presa durante
//! as esperas.

use std::{future::Future, sync::Mutex, time::Duration};

// O relógio do tokio (igual ao do sistema fora dos testes com tempo pausado).
use tokio::{sync::Notify, time::Instant};

use crate::http::{HttpError, HttpResult};

/// Intervalo entre listagens da indexação em segundo plano.
const BACKGROUND_GAP: Duration = Duration::from_millis(400);
/// Intervalo antes de uma requisição da interface (curto: o usuário está esperando).
const INTERACTIVE_GAP: Duration = Duration::from_millis(100);
const MAX_ATTEMPTS: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Priority {
    Interactive,
    Background,
}

impl Priority {
    fn gap(self) -> Duration {
        match self {
            Self::Interactive => INTERACTIVE_GAP,
            Self::Background => BACKGROUND_GAP,
        }
    }
}

/// Espera antes de repetir uma requisição recusada com 429: 5 s, 10 s, 20 s, 40 s, 60 s, 60 s…
pub fn backoff(attempt: u32) -> Duration {
    Duration::from_secs((5u64 << attempt.min(4)).min(60))
}

/// Quanto falta para respeitar o intervalo desde a última requisição.
fn remaining(last: Option<Instant>, now: Instant, gap: Duration) -> Duration {
    last.map_or(Duration::ZERO, |l| gap.saturating_sub(now.saturating_duration_since(l)))
}

#[derive(Default)]
struct State {
    /// Fim da última requisição.
    last: Option<Instant>,
    /// Há uma requisição em andamento.
    busy: bool,
    /// Chamadas da interface esperando a vez.
    interactive_waiting: usize,
    /// Recuo de 429 em vigor até este instante.
    backoff_until: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Go,
    /// Esperar este tempo e conferir de novo.
    Sleep(Duration),
    /// Esperar outra chamada terminar (aviso do `Notify`).
    Yield,
}

fn decide(st: &State, now: Instant, prio: Priority) -> Decision {
    if st.busy || (prio == Priority::Background && st.interactive_waiting > 0) {
        return Decision::Yield;
    }
    if let Some(until) = st.backoff_until.filter(|u| *u > now) {
        return Decision::Sleep(until - now);
    }
    match remaining(st.last, now, prio.gap()) {
        d if d.is_zero() => Decision::Go,
        d => Decision::Sleep(d),
    }
}

#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<State>,
    changed: Notify,
}

/// Vez de fazer a requisição. Ao sair (mesmo cancelada) libera a próxima.
struct Turn<'a>(&'a Gate);

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        {
            let mut st = self.0.state.lock().unwrap();
            st.busy = false;
            st.last = Some(Instant::now());
        }
        self.0.changed.notify_waiters();
    }
}

/// Conta a chamada da interface como esperando enquanto ela não recebe a vez.
struct Waiting<'a>(&'a Gate, bool);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if self.1 {
            self.0.state.lock().unwrap().interactive_waiting -= 1;
            self.0.changed.notify_waiters();
        }
    }
}

impl Gate {
    /// Até quando a nuvem pediu pausa (429), se ainda estiver em vigor.
    pub(crate) fn backing_off_until(&self) -> Option<std::time::Instant> {
        self.state.lock().unwrap().backoff_until.filter(|u| *u > Instant::now()).map(Instant::into_std)
    }

    async fn turn(&self, prio: Priority) -> Turn<'_> {
        let interactive = prio == Priority::Interactive;
        let mut waiting = Waiting(self, interactive);
        if interactive {
            self.state.lock().unwrap().interactive_waiting += 1;
        }
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let decision = {
                let mut st = self.state.lock().unwrap();
                let d = decide(&st, Instant::now(), prio);
                if d == Decision::Go {
                    st.busy = true;
                    if interactive {
                        st.interactive_waiting -= 1;
                        waiting.1 = false;
                    }
                }
                d
            };
            match decision {
                Decision::Go => return Turn(self),
                Decision::Sleep(d) => tokio::time::sleep(d).await,
                Decision::Yield => notified.await,
            }
        }
    }

    fn note_429(&self, attempt: u32) {
        let until = Instant::now() + backoff(attempt);
        let mut st = self.state.lock().unwrap();
        st.backoff_until = Some(st.backoff_until.map_or(until, |u| u.max(until)));
    }

    /// Faz a requisição na vez certa; em 429 espera o recuo (sem segurar a vez) e repete.
    pub(crate) async fn run<T, F, Fut>(&self, prio: Priority, mut request: F) -> HttpResult<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = HttpResult<T>>,
    {
        let mut attempt = 0;
        loop {
            let result = {
                let _turn = self.turn(prio).await;
                request().await
            };
            match result {
                Err(HttpError::Status { status: 429, .. }) if attempt + 1 < MAX_ATTEMPTS => {
                    self.note_429(attempt);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(0), Duration::from_secs(5));
        assert_eq!(backoff(1), Duration::from_secs(10));
        assert_eq!(backoff(4), Duration::from_secs(60));
        assert_eq!(backoff(9), Duration::from_secs(60));
    }

    #[test]
    fn remaining_spacing() {
        let t = Instant::now();
        assert_eq!(remaining(None, t, BACKGROUND_GAP), Duration::ZERO);
        assert_eq!(remaining(Some(t), t + Duration::from_millis(100), BACKGROUND_GAP), Duration::from_millis(300));
        assert_eq!(remaining(Some(t), t + Duration::from_millis(500), BACKGROUND_GAP), Duration::ZERO);
    }

    #[test]
    fn interactive_goes_before_background() {
        let t = Instant::now();
        let st = State { last: Some(t), interactive_waiting: 1, ..Default::default() };
        let later = t + Duration::from_secs(1);
        assert_eq!(decide(&st, later, Priority::Background), Decision::Yield);
        assert_eq!(decide(&st, later, Priority::Interactive), Decision::Go);
    }

    #[test]
    fn busy_makes_everyone_wait() {
        let st = State { busy: true, ..Default::default() };
        assert_eq!(decide(&st, Instant::now(), Priority::Interactive), Decision::Yield);
        assert_eq!(decide(&st, Instant::now(), Priority::Background), Decision::Yield);
    }

    #[test]
    fn spacing_depends_on_priority() {
        let t = Instant::now();
        let st = State { last: Some(t), ..Default::default() };
        let at = t + Duration::from_millis(150);
        assert_eq!(decide(&st, at, Priority::Interactive), Decision::Go);
        assert_eq!(decide(&st, at, Priority::Background), Decision::Sleep(Duration::from_millis(250)));
    }

    #[test]
    fn backoff_holds_everyone() {
        let t = Instant::now();
        let st = State { backoff_until: Some(t + Duration::from_secs(5)), ..Default::default() };
        assert_eq!(decide(&st, t, Priority::Interactive), Decision::Sleep(Duration::from_secs(5)));
        assert_eq!(decide(&st, t + Duration::from_secs(6), Priority::Background), Decision::Go);
    }

    fn too_many() -> HttpError {
        HttpError::Status { status: 429, body: String::new() }
    }

    #[tokio::test(start_paused = true)]
    async fn retries_after_429_and_exposes_backoff() {
        let gate = Arc::new(Gate::default());
        let calls = Arc::new(AtomicU32::new(0));
        let task = tokio::spawn({
            let (gate, calls) = (gate.clone(), calls.clone());
            async move {
                gate.run(Priority::Interactive, || {
                    let n = calls.fetch_add(1, Ordering::SeqCst);
                    async move { if n == 0 { Err(too_many()) } else { Ok(n) } }
                })
                .await
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(gate.backing_off_until().is_some());
        assert_eq!(task.await.unwrap().unwrap(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_max_attempts() {
        let gate = Gate::default();
        let calls = AtomicU32::new(0);
        let r: HttpResult<()> = gate
            .run(Priority::Background, || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err(too_many()) }
            })
            .await;
        assert!(matches!(r, Err(HttpError::Status { status: 429, .. })));
        assert_eq!(calls.load(Ordering::SeqCst), MAX_ATTEMPTS);
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_interactive_call_skips_queued_background() {
        let gate = Arc::new(Gate::default());
        let order = Arc::new(Mutex::new(Vec::new()));
        // Uma requisição em andamento segura a vez.
        let first = gate.turn(Priority::Background).await;
        let spawn = |prio: Priority, tag: &'static str| {
            let (gate, order) = (gate.clone(), order.clone());
            tokio::spawn(async move {
                gate.run(prio, || {
                    order.lock().unwrap().push(tag);
                    async { Ok(()) }
                })
                .await
            })
        };
        let bg = spawn(Priority::Background, "fundo");
        tokio::task::yield_now().await;
        let fg = spawn(Priority::Interactive, "interface");
        tokio::task::yield_now().await;
        drop(first);
        fg.await.unwrap().unwrap();
        bg.await.unwrap().unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["interface", "fundo"]);
    }
}
