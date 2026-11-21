//! Opções do mpv do player (cache, decodificação, nada da interface do próprio mpv).

use higanbana_core::player::store::Settings;
use higanbana_gpui::video::PlayerOptions;

/// Buffer à frente oferecido nos ajustes (MiB).
pub const CACHES: [u32; 4] = [50, 100, 150, 300];
/// Buffer para trás oferecido nas configurações (MiB).
pub const BACK_CACHES: [u32; 4] = [0, 32, 64, 256];

/// Teto de tempo do buffer à frente: o limite em bytes sozinho deixa um
/// episódio leve guardar minutos.
const AHEAD_SECS: &str = "60";

/// Pasta do cache em disco do mpv; sobras de um fechamento forçado são apagadas.
fn disk_cache_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("higanbana-cache");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn mpv(s: &Settings) -> PlayerOptions {
    let mut o = PlayerOptions::default();
    o = if s.disk_cache {
        o.set("cache-on-disk", "yes").set("demuxer-cache-dir", disk_cache_dir().to_string_lossy())
    } else {
        o.set("cache-on-disk", "no")
    };
    o = o
        .set("vd-lavc-threads", s.decode_threads.to_string())
        // Render sw: o quadro decodificado volta para a CPU.
        .set("hwdec", "auto-copy")
        .set("hwdec-extra-frames", "2")
        .set("keep-open", "yes")
        .set("osc", "no")
        .set("osd-level", "0")
        .set("input-default-bindings", "no")
        .set("cache", "yes")
        .set("demuxer-max-bytes", format!("{}MiB", s.cache_mib))
        .set("demuxer-max-back-bytes", format!("{}MiB", s.back_cache_mib))
        .set("cache-secs", AHEAD_SECS)
        .set("demuxer-readahead-secs", AHEAD_SECS)
        .set("audio-buffer", "0.2")
        .set("sub-auto", "no");
    // Diagnóstico: `HIGANBANA_MPV="hwdec=no,..."` sobrescreve sem recompilar.
    if let Ok(extra) = std::env::var("HIGANBANA_MPV") {
        for kv in extra.split(',') {
            if let Some((k, v)) = kv.split_once('=') {
                o = o.set(k.trim(), v.trim());
            }
        }
    }
    o
}
