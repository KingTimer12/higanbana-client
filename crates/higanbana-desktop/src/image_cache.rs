//! Cache de imagens com teto de memória e descarte do menos usado (LRU).
//!
//! O peso de cada entrada é o tamanho decodificado (largura × altura × 4),
//! que é o que realmente ocupa memória depois que o gpui decodifica a imagem.

use std::{cell::Cell, collections::HashMap, io::Cursor};

/// Teto do cache de imagens decodificadas.
pub const IMAGE_BUDGET: u64 = 384 * 1024 * 1024;

struct Entry<V> {
    value: V,
    weight: u64,
    last: Cell<u64>,
}

pub struct ImageCache<V: Clone> {
    map: HashMap<String, Entry<V>>,
    clock: Cell<u64>,
    total: u64,
    budget: u64,
}

impl<V: Clone> ImageCache<V> {
    pub fn new(budget: u64) -> Self {
        Self { map: HashMap::new(), clock: Cell::new(0), total: 0, budget }
    }

    fn tick(&self) -> u64 {
        let t = self.clock.get() + 1;
        self.clock.set(t);
        t
    }

    /// Devolve o valor e marca a entrada como usada agora.
    pub fn get(&self, key: &str) -> Option<V> {
        let e = self.map.get(key)?;
        e.last.set(self.tick());
        Some(e.value.clone())
    }

    /// Guarda o valor e devolve o que saiu: o valor substituído, se a chave já existia, e os
    /// descartados para caber no teto; o recém-inserido nunca sai.
    pub fn insert(&mut self, key: String, value: V, weight: u64) -> Vec<V> {
        let last = Cell::new(self.tick());
        let mut out = Vec::new();
        if let Some(old) = self.map.insert(key.clone(), Entry { value, weight, last }) {
            self.total -= old.weight;
            out.push(old.value);
        }
        self.total += weight;
        while self.total > self.budget {
            let victim = self
                .map
                .iter()
                .filter(|(k, e)| **k != key && e.weight > 0)
                .min_by_key(|(_, e)| e.last.get())
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            if let Some(e) = self.map.remove(&victim) {
                self.total -= e.weight;
                out.push(e.value);
            }
        }
        out
    }

    /// Guarda sem contar no teto e sem nunca ser descartado (peso 0): só sai por `remove` ou
    /// substituição. Para o que é caro de buscar de novo (bytes da nuvem).
    pub fn insert_pinned(&mut self, key: String, value: V) -> Vec<V> {
        self.insert(key, value, 0)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        let e = self.map.remove(key)?;
        self.total -= e.weight;
        Some(e.value)
    }
}

/// largura × altura × 4 lidos do cabeçalho; sem cabeçalho legível, bytes.len() × 4.
pub fn decoded_weight(bytes: &[u8]) -> u64 {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
        .map(|(w, h)| w as u64 * h as u64 * 4)
        .unwrap_or(bytes.len() as u64 * 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut c = ImageCache::new(100);
        c.insert("a".into(), 1, 40);
        c.insert("b".into(), 2, 40);
        c.get("a");
        let out = c.insert("c".into(), 3, 40);
        assert_eq!(out, vec![2]);
        assert!(c.get("a").is_some() && c.get("c").is_some() && c.get("b").is_none());
    }

    #[test]
    fn zero_weight_never_evicts() {
        let mut c = ImageCache::new(10);
        for i in 0..20 {
            assert!(c.insert(format!("k{i}"), i, 0).is_empty());
        }
        assert!(c.get("k0").is_some());
    }

    #[test]
    fn new_entry_survives_even_if_over_budget() {
        let mut c = ImageCache::new(10);
        c.insert("x".into(), 1, 50);
        assert_eq!(c.get("x"), Some(1));
    }

    #[test]
    fn pinned_never_evicted_nor_counted() {
        let mut c = ImageCache::new(100);
        assert!(c.insert_pinned("p".into(), 0).is_empty());
        assert!(c.insert("a".into(), 1, 100).is_empty());
        let out = c.insert("b".into(), 2, 100);
        assert_eq!(out, vec![1]);
        assert!(c.get("p").is_some());
    }

    #[test]
    fn remove_returns_value_and_frees_budget() {
        let mut c = ImageCache::new(100);
        c.insert("a".into(), 1, 60);
        assert_eq!(c.remove("a"), Some(1));
        assert_eq!(c.remove("a"), None);
        assert!(c.insert("b".into(), 2, 100).is_empty());
    }

    #[test]
    fn overwrite_returns_old_value_and_adjusts_total() {
        let mut c = ImageCache::new(100);
        c.insert("a".into(), 1, 90);
        assert_eq!(c.insert("a".into(), 2, 10), vec![1]);
        assert!(c.insert("b".into(), 3, 90).is_empty());
        assert_eq!(c.get("a"), Some(2));
    }

    #[test]
    fn weight_from_header() {
        let mut png = Vec::new();
        image::DynamicImage::new_rgba8(10, 20)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(decoded_weight(&png), 800);
    }
}
