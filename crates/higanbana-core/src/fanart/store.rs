//! Pasta de capas já reduzidas no disco, com teto de tamanho (best effort).

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use sha2::{Digest, Sha256};

/// Teto da pasta de capas; acima disso as mais antigas (por mtime) são apagadas.
pub const COVERS_MAX_BYTES: u64 = 300 * 1024 * 1024;

const EXTS: [&str; 2] = ["webp", "jpg"];

pub struct CoverStore {
    dir: PathBuf,
}

impl CoverStore {
    /// Cria a pasta, remove `.tmp` de gravações interrompidas e aplica o teto.
    pub fn open(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let store = Self { dir };
        store.prune(COVERS_MAX_BYTES);
        store
    }

    fn stem(url: &str) -> String {
        let digest = Sha256::digest(url.as_bytes());
        digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }

    /// Arquivo existente para a URL (`.webp` ou `.jpg`), se houver.
    pub fn path_for(&self, url: &str) -> Option<PathBuf> {
        let stem = Self::stem(url);
        EXTS.iter()
            .map(|e| self.dir.join(format!("{stem}.{e}")))
            .find(|p| p.is_file())
    }

    /// Grava via `.tmp` + rename para nunca expor arquivo parcial.
    pub fn save(&self, url: &str, bytes: &[u8], ext: &str) -> std::io::Result<PathBuf> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(format!("{}.{ext}", Self::stem(url)));
        let tmp = self.dir.join(format!("{}.{ext}.tmp", Self::stem(url)));
        fs::write(&tmp, bytes)?;
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(path)
    }

    /// Apaga um arquivo inválido (corrompido).
    pub fn discard(&self, path: &Path) {
        let _ = fs::remove_file(path);
    }

    fn prune(&self, max: u64) {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        for entry in rd.flatten() {
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("tmp") => {
                    let _ = fs::remove_file(&path);
                }
                Some(e) if EXTS.contains(&e) => {
                    if let Ok(meta) = entry.metadata() {
                        let at = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                        files.push((at, meta.len(), path));
                    }
                }
                _ => {}
            }
        }
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.0);
        for (_, len, path) in files {
            if total <= max {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, SystemTime};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    /// Pasta única por teste, apagada ao sair de escopo.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "higanbana-store-{}-{n}-{nanos}",
                std::process::id()
            )))
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn save_then_path_for() {
        let t = Tmp::new();
        let s = CoverStore::open(t.0.clone());
        let p = s.save("https://x/a.jpg", b"..", "webp").unwrap();
        assert_eq!(s.path_for("https://x/a.jpg"), Some(p));
    }

    #[test]
    fn different_urls_different_files() {
        let t = Tmp::new();
        let s = CoverStore::open(t.0.clone());
        let a = s.save("https://x/a.jpg", b"a", "webp").unwrap();
        let b = s.save("https://x/b.jpg", b"b", "jpg").unwrap();
        assert_ne!(a, b);
        assert_eq!(s.path_for("https://x/b.jpg"), Some(b));
    }

    #[test]
    fn tmp_leftover_not_returned() {
        let t = Tmp::new();
        let s = CoverStore::open(t.0.clone());
        let p = s.save("https://x/a.jpg", b"a", "webp").unwrap();
        let tmp = p.with_extension("webp.tmp");
        std::fs::rename(&p, &tmp).unwrap();
        assert_eq!(s.path_for("https://x/a.jpg"), None);
    }

    #[test]
    fn prune_drops_oldest() {
        let t = Tmp::new();
        let s = CoverStore::open(t.0.clone());
        let base = SystemTime::now() - Duration::from_secs(1000);
        let mut ps = vec![];
        for i in 0..3u64 {
            let p = s
                .save(&format!("https://x/{i}"), &[0u8; 10], "webp")
                .unwrap();
            let f = std::fs::File::options().write(true).open(&p).unwrap();
            f.set_modified(base + Duration::from_secs(i * 10)).unwrap();
            ps.push(p);
        }
        s.prune(20);
        assert!(!ps[0].exists());
        assert!(ps[1].exists());
        assert!(ps[2].exists());
    }

    #[test]
    fn open_removes_stale_tmp() {
        let t = Tmp::new();
        std::fs::create_dir_all(&t.0).unwrap();
        let tmp = t.0.join("abc.webp.tmp");
        std::fs::write(&tmp, b"x").unwrap();
        let _s = CoverStore::open(t.0.clone());
        assert!(!tmp.exists());
    }
}
