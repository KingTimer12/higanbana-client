//! Cache de imagens geradas (ícones, rampas de gradiente) com despejo por idade:
//! entrada não usada há `MAX_IDLE_FRAMES` frames sai, e a textura vai junto no
//! próximo `ImageCache::prepare` (que solta fontes sem outros donos).
use crate::element::ImageSource;
use std::cell::Cell;
use std::collections::HashMap;
use std::hash::Hash;

/// ~2 s a 60 fps.
pub const MAX_IDLE_FRAMES: u64 = 120;

thread_local! {
    static FRAME: Cell<u64> = const { Cell::new(0) };
    /// Varreduras registradas pelos caches da thread.
    static SWEEPS: std::cell::RefCell<Vec<fn(u64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn frame() -> u64 {
    FRAME.with(|f| f.get())
}

/// Fim de frame: avança o relógio e despeja o que ficou velho em todos os caches.
pub fn end_frame() {
    let now = FRAME.with(|f| {
        f.set(f.get() + 1);
        f.get()
    });
    let sweeps = SWEEPS.with(|s| s.borrow().clone());
    for sweep in sweeps {
        sweep(now);
    }
}

pub(crate) fn register(sweep: fn(u64)) {
    SWEEPS.with(|s| {
        let mut s = s.borrow_mut();
        if !s.contains(&sweep) {
            s.push(sweep);
        }
    });
}

/// Mapa chave → (valor, último frame usado).
pub(crate) struct AgedMap<K, T = Option<ImageSource>> {
    map: HashMap<K, (T, u64)>,
}

impl<K, T> Default for AgedMap<K, T> {
    fn default() -> Self {
        AgedMap { map: HashMap::new() }
    }
}

impl<K: Hash + Eq, T: Clone> AgedMap<K, T> {
    pub fn get(&mut self, k: &K) -> Option<T> {
        let now = frame();
        self.map.get_mut(k).map(|(v, t)| {
            *t = now;
            v.clone()
        })
    }

    pub fn insert(&mut self, k: K, v: T) {
        self.map.insert(k, (v, frame()));
    }

    pub fn sweep(&mut self, now: u64) {
        self.map.retain(|_, (_, t)| now.saturating_sub(*t) <= MAX_IDLE_FRAMES);
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}
