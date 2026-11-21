//! Tarefas em segundo plano: `work` em outra thread, `done` na thread da UI com a view.
use super::task::Tasks;
use super::EventCx;
use crate::gfx::Waker;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct V(Vec<u32>);

const WAIT: Duration = Duration::from_secs(5);

#[test]
fn task_result_delivered() {
    let mut tasks = Tasks::default();
    let mut cx = EventCx::default();
    cx.spawn(|| 42u32, |v: &mut V, r, cx| {
        v.0.push(r);
        cx.notify();
    });
    tasks.absorb(&mut cx, &Waker::noop());
    assert_eq!(tasks.len(), 1);
    let mut v = V::default();
    let out = tasks.drain_blocking(&mut v, WAIT).expect("resultado entregue");
    assert_eq!(v.0, vec![42]);
    assert!(out.is_notified());
    assert!(tasks.is_empty());
}

#[test]
fn work_runs_off_ui_thread() {
    let ui = std::thread::current().id();
    let mut tasks = Tasks::default();
    let mut cx = EventCx::default();
    cx.spawn(move || std::thread::current().id() != ui, |v: &mut V, other, _| v.0.push(other as u32));
    tasks.absorb(&mut cx, &Waker::noop());
    let mut v = V::default();
    tasks.drain_blocking(&mut v, WAIT);
    assert_eq!(v.0, vec![1]);
}

#[test]
fn cancelled_task_dropped() {
    let mut tasks = Tasks::default();
    let mut cx = EventCx::default();
    let id = cx.spawn(|| 1u32, |v: &mut V, r, _| v.0.push(r));
    cx.cancel_task(id);
    tasks.absorb(&mut cx, &Waker::noop());
    let mut v = V::default();
    assert!(tasks.drain_blocking(&mut v, Duration::from_millis(300)).is_none());
    assert!(v.0.is_empty());
    assert!(tasks.is_empty());
}

#[test]
fn task_after_close_dropped() {
    let finished = Arc::new(AtomicBool::new(false));
    let f = finished.clone();
    {
        let mut tasks = Tasks::default();
        let mut cx = EventCx::default();
        cx.spawn(
            move || {
                std::thread::sleep(Duration::from_millis(50));
                f.store(true, Ordering::SeqCst);
            },
            |_: &mut V, _, _| panic!("view já fechou"),
        );
        tasks.absorb(&mut cx, &Waker::noop());
    }
    let t0 = std::time::Instant::now();
    while !finished.load(Ordering::SeqCst) && t0.elapsed() < WAIT {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(finished.load(Ordering::SeqCst), "work terminou sem pânico");
}
