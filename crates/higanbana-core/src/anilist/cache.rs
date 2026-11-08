use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

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
    path: PathBuf,
    ttl_secs: i64,
    map: Mutex<HashMap<String, Entry<V>>>,
}

impl<V: Clone + Serialize + DeserializeOwned> DiskCache<V> {
    pub fn open(path: PathBuf, ttl_secs: i64) -> Self {
        let map = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<HashMap<String, Entry<V>>>(&b).ok())
            .unwrap_or_default();
        Self { path, ttl_secs, map: Mutex::new(map) }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let map = self.map.lock().unwrap();
        map.get(key)
            .filter(|e| now() - e.at < self.ttl_secs)
            .map(|e| e.value.clone())
    }

    /// Primeiro valor válido (não expirado) que satisfaz `pred`.
    pub fn find(&self, pred: impl Fn(&V) -> bool) -> Option<V> {
        let map = self.map.lock().unwrap();
        let t = now();
        map.values().filter(|e| t - e.at < self.ttl_secs).map(|e| &e.value).find(|v| pred(v)).cloned()
    }

    pub fn insert(&self, key: String, value: V) {
        self.map.lock().unwrap().insert(key, Entry { at: now(), value });
    }

    /// Grava em disco descartando entradas expiradas.
    pub fn flush(&self) {
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
}
