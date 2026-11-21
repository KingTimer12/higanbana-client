//! Caixa de correio: outras threads mandam closures que rodam na thread da UI
//! com a view (eventos do núcleo, rede, mídia). Cada envio acorda o laço.
use super::event::EventCx;
use crate::gfx::Waker;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

type Msg<V> = Box<dyn FnOnce(&mut V, &mut EventCx) + Send>;

/// Lado que envia: clonável e enviável entre threads.
pub struct Mailbox<V> {
    tx: Sender<Msg<V>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<V> Clone for Mailbox<V> {
    fn clone(&self) -> Self {
        Mailbox { tx: self.tx.clone(), waker: self.waker.clone() }
    }
}

impl<V: 'static> Mailbox<V> {
    /// Manda `f` para rodar na UI. `false` se a janela já fechou.
    pub fn send(&self, f: impl FnOnce(&mut V, &mut EventCx) + Send + 'static) -> bool {
        if self.tx.send(Box::new(f)).is_err() {
            return false;
        }
        // Antes da janela abrir não há waker: a mensagem roda no primeiro tick.
        if let Some(w) = self.waker.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            w.wake();
        }
        true
    }
}

/// Lado que recebe (fica com o runner).
pub struct Inbox<V> {
    rx: Receiver<Msg<V>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

pub(crate) fn inbox<V>() -> (Mailbox<V>, Inbox<V>) {
    let (tx, rx) = channel();
    let waker = Arc::new(Mutex::new(None));
    (Mailbox { tx, waker: waker.clone() }, Inbox { rx, waker })
}

impl<V> Inbox<V> {
    /// A janela abriu: envios passam a acordá-la.
    pub(crate) fn attach(&mut self, w: Waker) {
        *self.waker.lock().unwrap_or_else(|e| e.into_inner()) = Some(w);
    }

    /// Roda as mensagens pendentes. `None` se não havia nenhuma.
    pub(crate) fn drain(&mut self, view: &mut V) -> Option<EventCx> {
        let mut cx = None;
        while let Ok(f) = self.rx.try_recv() {
            f(view, cx.get_or_insert_with(EventCx::default));
        }
        cx
    }
}
