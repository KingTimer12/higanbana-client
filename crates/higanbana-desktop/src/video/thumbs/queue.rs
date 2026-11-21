//! Fila das miniaturas (sem mpv): o que pedir em seguida.

use std::collections::HashSet;

/// Uma miniatura a cada tanto de vídeo.
pub const STEP: f64 = 10.0;
/// Vizinhos pré-buscados para cada lado do último hover.
const AHEAD: u32 = 3;

pub fn bucket(t: f64) -> u32 {
    (t.max(0.0) / STEP).floor() as u32
}

/// Balde mais perto de `center` ainda sem miniatura, até `radius` (à frente primeiro).
pub fn nearest_missing(center: u32, last: u32, radius: u32, have: impl Fn(u32) -> bool) -> Option<u32> {
    (0..=radius)
        .flat_map(|d| [center.checked_add(d), (d > 0).then(|| center.checked_sub(d)).flatten()])
        .flatten()
        .filter(|b| *b <= last)
        .find(|b| !have(*b))
}

#[derive(Default)]
pub struct Queue {
    busy: Option<u32>,
    want: Option<u32>,
    near: Option<u32>,
    last: u32,
    done: HashSet<u32>,
}

impl Queue {
    pub fn set_len(&mut self, secs: f64) {
        self.last = bucket(secs);
    }

    pub fn want(&mut self, b: u32) {
        self.near = Some(b);
        self.want = (!self.done.contains(&b)).then_some(b);
    }

    /// Próximo balde a gerar (marca como em andamento), se nenhum estiver.
    pub fn next_job(&mut self) -> Option<u32> {
        if self.busy.is_some() {
            return None;
        }
        let next = self
            .want
            .filter(|b| !self.done.contains(b))
            .or_else(|| self.near.and_then(|c| nearest_missing(c, self.last, AHEAD, |b| self.done.contains(&b))))?;
        self.busy = Some(next);
        Some(next)
    }

    /// Terminou `b`. Falha também conta como feito (não repete em laço).
    pub fn finish(&mut self, b: u32, _ok: bool) {
        self.done.insert(b);
        if self.busy == Some(b) {
            self.busy = None;
        }
        if self.want == Some(b) {
            self.want = None;
        }
    }
}
