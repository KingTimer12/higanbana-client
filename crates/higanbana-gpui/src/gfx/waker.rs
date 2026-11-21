//! Acordar o laço da UI a partir de outra thread (quadro de vídeo, rede...).
use higanbana_gpui_sys as sys;
use std::sync::{Arc, Mutex};

/// Ponteiro da janela protegido: a janela zera ao morrer, então `wake` vira no-op.
pub(crate) struct WakeTarget(Mutex<usize>);

/// Clonável e enviável entre threads. `wake` faz o app reconstruir a view.
#[derive(Clone)]
pub struct Waker(Option<Arc<WakeTarget>>);

impl Waker {
    pub(crate) fn new(win: *mut sys::hg_window) -> (Self, Arc<WakeTarget>) {
        let t = Arc::new(WakeTarget(Mutex::new(win as usize)));
        (Waker(Some(t.clone())), t)
    }

    /// Não acorda nada (testes, sem janela).
    pub fn noop() -> Self {
        Waker(None)
    }

    pub fn wake(&self) {
        let Some(t) = &self.0 else { return };
        let guard = t.0.lock().unwrap_or_else(|e| e.into_inner());
        if *guard != 0 {
            // SAFETY: a janela só é destruída depois de zerar o alvo sob este mesmo lock.
            unsafe { sys::hg_window_wake(*guard as *mut sys::hg_window) };
        }
    }
}

impl WakeTarget {
    pub(crate) fn clear(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waker")
    }
}
