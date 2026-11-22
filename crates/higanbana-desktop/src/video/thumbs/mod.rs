//! Miniaturas da barra de posição: um segundo mpv sem vídeo na tela (`Player::headless`)
//! numa thread própria tira um JPEG a cada balde de 10 s. Um pedido por vez, o último
//! vence; parado, pré-busca os vizinhos.

mod queue;
mod worker;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use higanbana_gpui::prelude::ImageSource;
use higanbana_gpui::ImageData;

use crate::image_cache::Decoded;
pub use queue::bucket;
use queue::Queue;
pub use worker::{MpvShooter, Shooter};

/// Avisa a UI (outra thread): `(balde, quadro)`; a UI chama `Thumbs::finish`.
pub type Done = Box<dyn Fn(u32, Option<Decoded>) + Send>;

/// Pasta própria de cada gerador (um fechado ainda pode estar apagando a dele).
pub fn thumbs_dir(seq: u64) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join("higanbana-thumbs").join(format!("{}-{seq}-{n}", std::process::id()))
}

pub struct Thumbs {
    queue: Queue,
    jobs: Sender<u32>,
    done: HashMap<u32, ImageSource>,
    /// Ligado ao soltar: a abertura em curso (até 20 s) desiste e o mpv morre logo.
    cancel: Arc<AtomicBool>,
}

impl Drop for Thumbs {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Thumbs {
    /// Abre `url` num mpv headless; fotos em `dir` (apagada ao fechar).
    pub fn open(url: String, dir: PathBuf, on_done: impl Fn(u32, Option<Decoded>) + Send + 'static) -> Thumbs {
        let d = dir.clone();
        Thumbs::with_shooter(move |cancel| MpvShooter::open(&url, d, cancel).map(|s| Box::new(s) as Box<dyn Shooter>), on_done, dir)
    }

    /// Com outro "fotógrafo" (testes).
    pub fn with_shooter(
        make: impl FnOnce(&AtomicBool) -> Result<Box<dyn Shooter>, String> + Send + 'static,
        on_done: impl Fn(u32, Option<Decoded>) + Send + 'static,
        dir: PathBuf,
    ) -> Thumbs {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        worker::spawn(make, cancel.clone(), rx, Box::new(on_done), dir);
        Thumbs { queue: Queue::default(), jobs: tx, done: HashMap::new(), cancel }
    }

    pub fn set_len(&mut self, secs: f64) {
        self.queue.set_len(secs);
    }

    /// Pede a miniatura do tempo `t` (segundos).
    pub fn want(&mut self, t: f64) {
        self.queue.want(bucket(t));
        self.pump();
    }

    pub fn get(&self, t: f64) -> Option<ImageSource> {
        self.done.get(&bucket(t)).cloned()
    }

    /// Resposta do worker (na thread da UI).
    pub fn finish(&mut self, b: u32, frame: Option<Decoded>) {
        let ok = frame.is_some();
        if let Some(f) = frame {
            self.done.insert(b, ImageData::from_rgba8(f.width, f.height, f.rgba));
        }
        self.queue.finish(b, ok);
        self.pump();
    }

    fn pump(&mut self) {
        if let Some(b) = self.queue.next_job()
            && self.jobs.send(b).is_err()
        {
            self.queue.finish(b, false);
        }
    }
}
