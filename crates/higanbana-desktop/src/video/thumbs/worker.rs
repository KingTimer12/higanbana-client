//! Thread das miniaturas: dona do mpv headless; atende o pedido mais recente.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use higanbana_gpui::video::{Player, PlayerOptions, Status};

use super::queue::STEP;
use super::Done;
use crate::image_cache::{self, Decoded};

const WIDTH: u32 = 320;

/// Tira a foto de um balde.
pub trait Shooter {
    fn shoot(&mut self, bucket: u32) -> Option<Decoded>;
}

fn options() -> PlayerOptions {
    [
        ("vo", "null"),
        ("pause", "yes"),
        ("keep-open", "yes"),
        ("idle", "yes"),
        ("audio", "no"),
        ("sid", "no"),
        ("hwdec", "no"),
        ("hr-seek", "no"),
        ("vd-lavc-threads", "1"),
        ("vd-lavc-skiploopfilter", "all"),
        ("vd-lavc-fast", "yes"),
        ("demuxer-lavf-probesize", "524288"),
        ("demuxer-lavf-analyzeduration", "0.5"),
        ("cache", "no"),
        ("demuxer-readahead-secs", "0"),
        ("demuxer-max-bytes", "4MiB"),
        ("screenshot-format", "jpg"),
        ("screenshot-jpeg-quality", "80"),
        ("config", "no"),
        ("load-scripts", "no"),
        ("ytdl", "no"),
        ("input-default-bindings", "no"),
        ("terminal", "no"),
    ]
    .into_iter()
    .fold(PlayerOptions::empty(), |o, (k, v)| o.set(k, v))
    .set("vf", format!("scale=w={WIDTH}:h=-2"))
}

fn until(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

pub struct MpvShooter {
    player: Player,
    dir: PathBuf,
}

impl MpvShooter {
    /// Abre o vídeo e espera carregar (até 20 s).
    pub fn open(url: &str, dir: PathBuf) -> Result<MpvShooter, String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let player = Player::headless(options())?;
        player.load(url);
        let ok = until(20, || matches!(player.state().status, Status::Paused | Status::Playing | Status::Ended | Status::Error(_)));
        if !ok || matches!(player.state().status, Status::Error(_)) {
            return Err("miniaturas: vídeo não abriu".into());
        }
        Ok(MpvShooter { player, dir })
    }
}

impl Shooter for MpvShooter {
    fn shoot(&mut self, bucket: u32) -> Option<Decoded> {
        let t = f64::from(bucket) * STEP;
        self.player.command_blocking(&["seek", &t.to_string(), "absolute+keyframes"]).ok()?;
        // O seek é assíncrono no mpv: espera a posição chegar perto do balde.
        until(3, || (self.player.state().position - t).abs() < STEP);
        let path = self.dir.join(format!("{bucket}.jpg"));
        self.player.command_blocking(&["screenshot-to-file", &path.to_string_lossy(), "video"]).ok()?;
        let bytes = std::fs::read(&path).ok()?;
        let _ = std::fs::remove_file(&path);
        image_cache::decode(&bytes, WIDTH, WIDTH).map(|(d, _)| d)
    }
}

/// Roda até o `Sender` cair; pega sempre o pedido mais recente da fila.
pub fn spawn(make: impl FnOnce() -> Result<Box<dyn Shooter>, String> + Send + 'static, jobs: Receiver<u32>, done: Done, dir: PathBuf) {
    let _ = std::thread::Builder::new().name("miniaturas".into()).spawn(move || {
        let mut shooter = match make() {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::debug!(%e, "miniaturas desligadas");
                None
            }
        };
        while let Ok(mut b) = jobs.recv() {
            while let Ok(newer) = jobs.try_recv() {
                done(b, None);
                b = newer;
            }
            let frame = shooter.as_mut().and_then(|s| s.shoot(b));
            done(b, frame);
        }
        drop(shooter);
        let _ = std::fs::remove_dir_all(&dir);
    });
}
