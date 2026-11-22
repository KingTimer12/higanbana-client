//! Medição de texto para o layout, com cache por (texto, fonte, tamanho, largura).
use crate::geometry::Size;
use crate::style::TextStyle;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};

/// Fonte de medidas de texto em pixels lógicos. `max_width = None` não quebra linha.
/// Abstraído para o layout rodar em testes sem GPU.
pub trait TextMeasurer {
    fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> Size;

    /// x (lógico) de cada fronteira de caractere numa linha: `codepoints + 1` valores.
    /// O padrão mede prefixos (lento); o medidor de GPU usa um layout só.
    fn caret_positions(&mut self, text: &str, style: &TextStyle, out: &mut Vec<f32>) {
        out.clear();
        out.push(0.0);
        for (i, c) in text.char_indices() {
            out.push(self.measure(&text[..i + c.len_utf8()], style, None).width);
        }
    }
}

/// Hasher FNV-1a: chaves curtas, sem a sobrecarga do SipHash.
#[derive(Default)]
pub(crate) struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut h = if self.0 == 0 { 0xcbf2_9ce4_8422_2325 } else { self.0 };
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.0 = h;
    }
}

/// Teto de entradas; ao estourar o cache é zerado (evita crescer sem limite).
const MAX_ENTRIES: usize = 4096;

/// Cache de medidas. A chave é um hash de 64 bits (sem guardar a string).
#[derive(Default)]
pub struct MeasureCache {
    map: HashMap<u64, Size, BuildHasherDefault<Fnv>>,
    misses: u32,
}

fn key(text: &str, style: &TextStyle, max_width: Option<f32>) -> u64 {
    let mut h = Fnv::default();
    text.hash(&mut h);
    style.font.hash(&mut h);
    style.size.to_bits().hash(&mut h);
    max_width.map(f32::to_bits).hash(&mut h);
    h.finish()
}

impl MeasureCache {
    pub fn measure(&mut self, m: &mut dyn TextMeasurer, text: &str, style: &TextStyle, max_width: Option<f32>) -> Size {
        let k = key(text, style, max_width);
        if let Some(s) = self.map.get(&k) {
            return *s;
        }
        if self.map.len() >= MAX_ENTRIES {
            self.map.clear();
        }
        self.misses += 1;
        let s = m.measure(text, style, max_width);
        self.map.insert(k, s);
        s
    }

    /// Esquece tudo (ex.: mudou a escala de DPI ou o backend).
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Quantas medições reais já foram feitas (para testes/diagnóstico).
    pub fn misses(&self) -> u32 {
        self.misses
    }
}

/// Altura de uma linha no estilo.
pub(crate) fn line_height(cache: &mut MeasureCache, m: &mut dyn TextMeasurer, style: &TextStyle) -> f32 {
    cache.measure(m, " ", style, None).height
}

/// Texto cortado para caber em `lines` linhas de largura `w`, terminando em "…".
/// `None` quando já cabe. Busca binária no número de caracteres (medidas em cache).
pub(crate) fn clamp_text(cache: &mut MeasureCache, m: &mut dyn TextMeasurer, text: &str, style: &TextStyle, w: f32, lines: u32) -> Option<String> {
    let limit = line_height(cache, m, style) * lines as f32 + 0.5;
    if cache.measure(m, text, style, Some(w)).height <= limit {
        return None;
    }
    let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())).collect();
    let cut = |k: usize| format!("{}…", text[..ends[k]].trim_end());
    let (mut lo, mut hi) = (0usize, ends.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if cache.measure(m, &cut(mid), style, Some(w)).height <= limit {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(cut(lo))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Medidor falso: 8 px por caractere, 16 px por linha, quebra por largura.
    pub(crate) struct FakeMeasurer;

    impl TextMeasurer for FakeMeasurer {
        fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> Size {
            let cw = style.size / 2.0;
            let full = text.chars().count() as f32 * cw;
            let line_h = style.size + 2.0;
            match max_width {
                Some(w) if w > 0.0 && full > w => {
                    let per_line = (w / cw).floor().max(1.0);
                    let lines = (text.chars().count() as f32 / per_line).ceil();
                    Size { width: per_line * cw, height: lines * line_h }
                }
                _ => Size { width: full, height: line_h },
            }
        }
    }

    #[test]
    fn cache_hits_skip_measurer() {
        let mut cache = MeasureCache::default();
        let style = TextStyle::default();
        let a = cache.measure(&mut FakeMeasurer, "olá", &style, None);
        let b = cache.measure(&mut FakeMeasurer, "olá", &style, None);
        assert_eq!(a, b);
        assert_eq!(cache.misses(), 1);
        cache.measure(&mut FakeMeasurer, "olá", &style, Some(10.0));
        let bigger = TextStyle { size: 20.0, ..style };
        cache.measure(&mut FakeMeasurer, "olá", &bigger, None);
        assert_eq!(cache.misses(), 3);
    }
}
