//! Cache de imagens com teto de memória e descarte do menos usado (LRU).
//!
//! O peso de cada entrada é o tamanho decodificado (largura × altura × 4),
//! que é o que realmente ocupa memória depois de decodificada.

use std::cell::Cell;

use rustc_hash::FxHashMap;

/// Teto do cache de imagens decodificadas.
pub const IMAGE_BUDGET: u64 = 64 * 1024 * 1024;

struct Entry<V> {
    value: V,
    weight: u64,
    last: Cell<u64>,
}

pub struct ImageCache<V: Clone> {
    map: FxHashMap<String, Entry<V>>,
    clock: Cell<u64>,
    total: u64,
    budget: u64,
}

impl<V: Clone> ImageCache<V> {
    pub fn new(budget: u64) -> Self {
        Self { map: FxHashMap::default(), clock: Cell::new(0), total: 0, budget }
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
        // O relógio só cresce: o tique identifica a entrada nova sem guardar uma cópia da chave.
        let now = self.tick();
        let mut out = Vec::new();
        if let Some(old) = self.map.insert(key, Entry { value, weight, last: Cell::new(now) }) {
            self.total -= old.weight;
            out.push(old.value);
        }
        self.total += weight;
        while self.total > self.budget {
            let victim = self
                .map
                .iter()
                .filter(|(_, e)| e.last.get() != now && e.weight > 0)
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

    /// Tira tudo que conta no teto (fica só o fixado com `insert_pinned`) e devolve
    /// os valores para serem soltos.
    pub fn drain_weighted(&mut self) -> Vec<V> {
        let keys: Vec<String> = self.map.iter().filter(|(_, e)| e.weight > 0).map(|(k, _)| k.clone()).collect();
        keys.into_iter().filter_map(|k| self.remove(&k)).collect()
    }

    pub fn total_mb(&self) -> u64 {
        self.total >> 20
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        let e = self.map.remove(key)?;
        self.total -= e.weight;
        Some(e.value)
    }
}

/// Maior lado de imagens da interface: capas em pé até `PORTRAIT_MAX` de altura
/// (o maior pôster na tela tem ~24 rem; 480 px cobre escala de 125%),
/// banners/stills até `LANDSCAPE_MAX` de largura. Fanart e TMDB chegam bem maiores
/// do que o card mostra, e decodificado cada pixel custa 4 bytes.
pub const PORTRAIT_MAX: u32 = 480;
pub const LANDSCAPE_MAX: u32 = 1600;
/// Páginas do leitor: precisam de nitidez em tela cheia.
pub const PAGE_MAX: u32 = 2400;

/// Pixels RGBA8 (alfa reto) decodificados fora da UI; viram `ImageSource` na UI.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodifica já reduzida ao teto, sem guardar os bytes originais. Devolve os
/// pixels e o peso em memória. Chamar fora da thread da UI. GIF: só o 1º quadro.
pub fn decode(bytes: &[u8], portrait_max: u32, landscape_max: u32) -> Option<(Decoded, u64)> {
    match try_decode(bytes, portrait_max, landscape_max) {
        Ok(r) => Some(r),
        Err(e) => {
            let head: String = bytes.iter().take(16).map(|b| format!("{b:02x}")).collect();
            tracing::warn!(error = %e, len = bytes.len(), head, "decodificação falhou");
            None
        }
    }
}

fn try_decode(bytes: &[u8], portrait_max: u32, landscape_max: u32) -> Result<(Decoded, u64), String> {
    use image::imageops::FilterType;
    let mut img = if crate::avif::is_avif(bytes) {
        image::DynamicImage::ImageRgba8(crate::avif::decode(bytes)?)
    } else {
        image::load_from_memory(bytes).map_err(|e| e.to_string())?
    };
    let (w, h) = (img.width(), img.height());
    let (mw, mh) = if h > w { (u32::MAX, portrait_max) } else { (landscape_max, u32::MAX) };
    if w > mw || h > mh {
        img = img.resize(mw.min(w), mh.min(h), FilterType::Triangle);
    }
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    let rgba = rgba.into_raw();
    let weight = rgba.len() as u64;
    Ok((Decoded { width, height, rgba }, weight))
}

#[cfg(test)]
mod drain_tests {
    use super::*;

    #[test]
    fn drain_keeps_pinned() {
        let mut c = ImageCache::new(100);
        c.insert("a".into(), 1, 10);
        c.insert_pinned("p".into(), 2);
        assert_eq!(c.drain_weighted(), vec![1]);
        assert_eq!(c.total_mb(), 0);
        assert_eq!(c.get("p"), Some(2));
        assert_eq!(c.get("a"), None);
    }
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
    fn textura_solta_ao_evictar() {
        // A lib só mantém a textura enquanto alguém além do cache dela segura o `ImageSource`.
        let src = |n: u32| higanbana_gpui::ImageData::from_rgba8(n, 1, vec![0; n as usize * 4]);
        let (a, b) = (src(1), src(2));
        let mut c = ImageCache::new(100);
        c.insert("a".into(), a.clone(), 60);
        drop(c.insert("b".into(), b.clone(), 60));
        assert_eq!(std::rc::Rc::strong_count(&a), 1, "a saiu do LRU: só o teste segura");
        assert_eq!(std::rc::Rc::strong_count(&b), 2);
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

    use std::io::Cursor;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::new_rgba8(w, h).write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn decode_keeps_small_images() {
        let (img, weight) = decode(&png(10, 20), 640, 1600).unwrap();
        assert_eq!(weight, 800);
        assert_eq!((img.width, img.height), (10, 20));
    }

    #[test]
    fn decode_shrinks_to_the_cap() {
        let (tall, _) = decode(&png(1000, 1426), 640, 1600).unwrap();
        assert_eq!(tall.height, 640);
        let (wide, _) = decode(&png(3840, 2160), 640, 1600).unwrap();
        assert_eq!((wide.width, wide.height), (1600, 900));
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode(b"nope", 640, 1600).is_none());
    }
}
