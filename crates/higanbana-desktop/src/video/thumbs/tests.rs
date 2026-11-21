use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::queue::{bucket, nearest_missing, Queue};
use super::{Shooter, Thumbs};
use crate::image_cache::Decoded;

#[test]
fn balde_de_10s() {
    assert_eq!(bucket(0.0), 0);
    assert_eq!(bucket(9.99), 0);
    assert_eq!(bucket(10.0), 1);
    assert_eq!(bucket(-3.0), 0);
}

#[test]
fn prefetch_mais_perto_primeiro() {
    let have = |set: &'static [u32]| move |b: u32| set.contains(&b);
    assert_eq!(nearest_missing(5, 100, 3, have(&[])), Some(5));
    assert_eq!(nearest_missing(5, 100, 3, have(&[5])), Some(6));
    assert_eq!(nearest_missing(5, 100, 3, have(&[5, 6])), Some(4));
    assert_eq!(nearest_missing(5, 100, 3, have(&[2, 3, 4, 5, 6, 7, 8])), None);
    assert_eq!(nearest_missing(2, 2, 3, |b| b == 2), Some(1), "nada depois do fim");
}

#[test]
fn ultimo_pedido_vence() {
    let mut q = Queue::default();
    q.set_len(1000.0);
    q.want(3);
    assert_eq!(q.next_job(), Some(3));
    q.want(7);
    q.want(9);
    assert_eq!(q.next_job(), None, "um por vez");
    q.finish(3, true);
    assert_eq!(q.next_job(), Some(9));
}

struct Slow(Arc<Mutex<u32>>);

impl Shooter for Slow {
    fn shoot(&mut self, _bucket: u32) -> Option<Decoded> {
        std::thread::sleep(Duration::from_millis(500));
        *self.0.lock().unwrap() += 1;
        None
    }
}

#[test]
fn drop_durante_geracao_nao_trava() {
    let shots = Arc::new(Mutex::new(0));
    let s = shots.clone();
    let mut th = Thumbs::with_shooter(move || Ok(Box::new(Slow(s)) as Box<dyn Shooter>), |_, _| {}, std::env::temp_dir().join("hg-thumbs-teste-drop"));
    th.set_len(100.0);
    th.want(20.0);
    std::thread::sleep(Duration::from_millis(50));
    let t0 = Instant::now();
    drop(th);
    assert!(t0.elapsed() < Duration::from_millis(50), "drop esperou o worker");
}

#[test]
fn mpv_tira_miniatura_de_320() {
    let dir = std::env::temp_dir().join(format!("hg-thumbs-mpv-{}", std::process::id()));
    let mut s = super::MpvShooter::open("av://lavfi:testsrc=duration=25:size=640x360:rate=24", dir.clone()).expect("libmpv");
    let f = s.shoot(1).expect("miniatura");
    assert_eq!((f.width, f.height), (320, 180));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pasta_unica_por_gerador() {
    assert_ne!(super::thumbs_dir(1), super::thumbs_dir(1));
}
