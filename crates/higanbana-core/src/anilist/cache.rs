use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Espera antes de gravar: pedidos seguidos (uma tela de capas, um lote do
/// AniList) viram uma gravação só.
const FLUSH_DELAY: Duration = Duration::from_secs(2);

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize)]
struct Entry<V> {
    at: i64,
    value: V,
}

/// Cache chave→valor com TTL, persistido como JSON (best effort).
pub struct DiskCache<V> {
    inner: Arc<Inner<V>>,
}

struct Inner<V> {
    path: PathBuf,
    ttl_secs: i64,
    map: Mutex<HashMap<String, Entry<V>>>,
    /// Mudou desde a última gravação.
    dirty: AtomicBool,
    /// Já há uma gravação agendada.
    scheduled: AtomicBool,
    /// Serializa as gravações (o `.tmp` é fixo).
    writing: Mutex<()>,
}

impl<V: Clone + Serialize + DeserializeOwned + Send + 'static> DiskCache<V> {
    pub fn open(path: PathBuf, ttl_secs: i64) -> Self {
        let map = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<HashMap<String, Entry<V>>>(&b).ok())
            .unwrap_or_default();
        let inner = Inner {
            path,
            ttl_secs,
            map: Mutex::new(map),
            dirty: AtomicBool::new(false),
            scheduled: AtomicBool::new(false),
            writing: Mutex::new(()),
        };
        Self { inner: Arc::new(inner) }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let map = self.inner.map.lock().unwrap();
        map.get(key)
            .filter(|e| now() - e.at < self.inner.ttl_secs)
            .map(|e| e.value.clone())
    }

    /// Primeiro valor válido (não expirado) que satisfaz `pred`.
    pub fn find(&self, pred: impl Fn(&V) -> bool) -> Option<V> {
        let map = self.inner.map.lock().unwrap();
        let t = now();
        map.values().filter(|e| t - e.at < self.inner.ttl_secs).map(|e| &e.value).find(|v| pred(v)).cloned()
    }

    pub fn insert(&self, key: String, value: V) {
        self.inner.map.lock().unwrap().insert(key, Entry { at: now(), value });
        self.inner.dirty.store(true, Ordering::Release);
    }

    /// Grava em disco descartando entradas expiradas; sem mudança, não faz nada.
    /// Dentro do runtime, a gravação sai daqui a `FLUSH_DELAY` numa thread de
    /// bloqueio (o arquivo inteiro é reescrito); fechar o app antes só perde cache.
    pub fn flush(&self) {
        if !self.inner.dirty.load(Ordering::Acquire) {
            return;
        }
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            self.inner.write();
            return;
        };
        if self.inner.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let inner = self.inner.clone();
        rt.spawn(async move {
            tokio::time::sleep(FLUSH_DELAY).await;
            // Antes de gravar: o que mudar durante a gravação agenda outra.
            inner.scheduled.store(false, Ordering::Release);
            let _ = tokio::task::spawn_blocking(move || inner.write()).await;
        });
    }
}

impl<V: Serialize> Inner<V> {
    fn write(&self) {
        let _writing = self.writing.lock().unwrap();
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return;
        }
        let bytes = {
            let mut map = self.map.lock().unwrap();
            let ttl = self.ttl_secs;
            let t = now();
            map.retain(|_, e| t - e.at < ttl);
            serde_json::to_vec(&*map)
        };
        if let Ok(bytes) = bytes {
            if let Some(dir) = self.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = self.path.with_extension("json.tmp");
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_by_value() {
        let cache: DiskCache<i64> = DiskCache::open(PathBuf::from("nao-existe/cache-teste.json"), 60);
        cache.insert("a".into(), 1);
        cache.insert("b".into(), 2);
        assert_eq!(cache.find(|v| *v == 2), Some(2));
        assert_eq!(cache.find(|v| *v == 3), None);
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("higanbana-cache-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("cache.json")
    }

    #[test]
    fn fora_do_runtime_grava_na_hora_e_so_se_mudou() {
        let path = tmp("sync");
        let cache: DiskCache<i64> = DiskCache::open(path.clone(), 60);
        cache.flush();
        assert!(!path.exists(), "sem mudança não grava");
        cache.insert("a".into(), 1);
        cache.flush();
        assert_eq!(DiskCache::<i64>::open(path, 60).get("a"), Some(1));
    }

    #[tokio::test(start_paused = true)]
    async fn no_runtime_junta_as_gravacoes() {
        let path = tmp("async");
        let cache: DiskCache<i64> = DiskCache::open(path.clone(), 60);
        cache.insert("a".into(), 1);
        cache.flush();
        cache.insert("b".into(), 2);
        cache.flush();
        assert!(!path.exists(), "a gravação espera FLUSH_DELAY");
        tokio::time::sleep(FLUSH_DELAY * 2).await;
        // A escrita roda numa thread de bloqueio: espera ela terminar.
        for _ in 0..100 {
            if path.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            tokio::task::yield_now().await;
        }
        let again = DiskCache::<i64>::open(path, 60);
        assert_eq!((again.get("a"), again.get("b")), (Some(1), Some(2)));
    }
}
