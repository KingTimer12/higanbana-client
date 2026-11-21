//! Miniaturas da barra de posição: um segundo mpv sem vídeo na tela (`Player::headless`)
//! numa thread própria tira um JPEG a cada balde de 10 s. Um pedido por vez, o último
//! vence; parado, pré-busca os vizinhos.

mod queue;
mod worker;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};

use higanbana_gpui::prelude::ImageSource;
use higanbana_gpui::ImageData;

use crate::image_cache::Decoded;
pub use queue::bucket;
use queue::Queue;
pub use worker::{MpvShooter, Shooter};

/// Avisa a UI (outra thread): `(balde, quadro)`; a UI chama `Thumbs::finish`.
pub type Done = Box<dyn Fn(u32, Option<Decoded>) + Send>;

pub struct Thumbs {
    queue: Queue,
    jobs: Sender<u32>,
    done: HashMap<u32, ImageSource>,
}

impl Thumbs {
    /// Abre `url` num mpv headless; fotos em `dir` (apagada ao fechar).
    pub fn open(url: String, dir: PathBuf, on_done: impl Fn(u32, Option<Decoded>) + Send + 'static) -> Thumbs {
        let d = dir.clone();
        Thumbs::with_shooter(move || MpvShooter::open(&url, d).map(|s| Box::new(s) as Box<dyn Shooter>), on_done, dir)
    }

    /// Com outro "fotógrafo" (testes).
    pub fn with_shooter(
        make: impl FnOnce() -> Result<Box<dyn Shooter>, String> + Send + 'static,
        on_done: impl Fn(u32, Option<Decoded>) + Send + 'static,
        dir: PathBuf,
    ) -> Thumbs {
        let (tx, rx) = channel();
        worker::spawn(make, rx, Box::new(on_done), dir);
        Thumbs { queue: Queue::default(), jobs: tx, done: HashMap::new() }
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
