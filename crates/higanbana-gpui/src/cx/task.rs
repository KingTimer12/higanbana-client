//! Tarefas em segundo plano: `work` roda num pool pequeno de threads; o resultado volta
//! pela fila e `done` roda na thread da UI com a view (o laço é acordado pelo `Waker`).
use super::event::EventCx;
use crate::gfx::Waker;
use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

/// Threads do pool (rede, decodificação, disco).
const WORKERS: usize = 4;

/// Identificador de tarefa (para `cancel_task`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        TaskId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

type Output = Box<dyn Any + Send>;
pub(crate) type WorkFn = Box<dyn FnOnce() -> Output + Send>;
pub(crate) type DoneFn = Box<dyn FnOnce(&mut dyn Any, Output, &mut EventCx)>;
type Job = Box<dyn FnOnce() + Send>;

/// Pedido feito por um handler/render, ainda não enviado ao pool.
pub(crate) struct TaskReq {
    pub id: TaskId,
    pub work: WorkFn,
    pub done: DoneFn,
}

pub(crate) fn request<V: 'static, T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(&mut V, T, &mut EventCx) + 'static,
) -> TaskReq {
    let work: WorkFn = Box::new(move || Box::new(work()) as Output);
    let done: DoneFn = Box::new(move |v: &mut dyn Any, out: Output, cx: &mut EventCx| {
        if let (Some(v), Ok(out)) = (v.downcast_mut::<V>(), out.downcast::<T>()) {
            done(v, *out, cx);
        }
    });
    TaskReq { id: TaskId::next(), work, done }
}

/// Pool global criado na primeira tarefa.
fn pool() -> &'static Mutex<Sender<Job>> {
    static POOL: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    POOL.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        let rx = std::sync::Arc::new(Mutex::new(rx));
        for i in 0..WORKERS {
            let rx = rx.clone();
            let _ = std::thread::Builder::new().name(format!("higanbana-task-{i}")).spawn(move || loop {
                let job = match rx.lock() {
                    Ok(r) => r.recv(),
                    Err(_) => return,
                };
                match job {
                    // Pânico no trabalho não derruba o worker; o resultado só não chega.
                    Ok(job) => drop(std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))),
                    Err(_) => return,
                }
            });
        }
        Mutex::new(tx)
    })
}

/// Tarefas em voo da janela: callbacks `done` por id e a fila de resultados.
pub struct Tasks {
    done: HashMap<TaskId, DoneFn>,
    tx: Sender<(TaskId, Output)>,
    rx: Receiver<(TaskId, Output)>,
}

impl Default for Tasks {
    fn default() -> Self {
        let (tx, rx) = channel();
        Tasks { done: HashMap::new(), tx, rx }
    }
}

impl Tasks {
    /// Envia ao pool os pedidos do contexto e descarta os cancelados.
    pub(crate) fn absorb(&mut self, cx: &mut EventCx, waker: &Waker) {
        for req in cx.tasks.drain(..) {
            self.submit(req, waker);
        }
        for id in cx.cancelled_tasks.drain(..) {
            self.done.remove(&id);
        }
    }

    pub(crate) fn submit(&mut self, req: TaskReq, waker: &Waker) {
        let TaskReq { id, work, done } = req;
        self.done.insert(id, done);
        let (tx, waker) = (self.tx.clone(), waker.clone());
        let job: Job = Box::new(move || {
            let out = work();
            // Janela fechada: o receptor sumiu e o resultado é descartado.
            if tx.send((id, out)).is_ok() {
                waker.wake();
            }
        });
        if let Ok(p) = pool().lock() {
            let _ = p.send(job);
        }
    }

    /// Tarefas cujo `done` ainda não rodou.
    pub fn len(&self) -> usize {
        self.done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }

    /// Entrega os resultados prontos (sem bloquear). `None` se nada chegou.
    pub(crate) fn drain(&mut self, view: &mut dyn Any) -> Option<EventCx> {
        let mut cx = None;
        while let Ok((id, out)) = self.rx.try_recv() {
            self.deliver(id, out, view, &mut cx);
        }
        cx
    }

    fn deliver(&mut self, id: TaskId, out: Output, view: &mut dyn Any, cx: &mut Option<EventCx>) {
        // Cancelada: sem callback, resultado descartado.
        if let Some(done) = self.done.remove(&id) {
            done(view, out, cx.get_or_insert_with(EventCx::default));
        }
    }

    /// Espera até `timeout` por um resultado e entrega o que houver (testes).
    #[cfg(test)]
    pub(crate) fn drain_blocking(&mut self, view: &mut dyn Any, timeout: std::time::Duration) -> Option<EventCx> {
        let mut cx = None;
        let t0 = std::time::Instant::now();
        while cx.is_none() && !self.done.is_empty() {
            let left = timeout.saturating_sub(t0.elapsed());
            match self.rx.recv_timeout(left) {
                Ok((id, out)) => self.deliver(id, out, view, &mut cx),
                Err(_) => break,
            }
        }
        cx
    }
}
