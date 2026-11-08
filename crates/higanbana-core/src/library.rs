//! Como o acervo da nuvem se organiza: seções, pastas de título, nomes de
//! episódio e ordem natural. Tudo puro, sem rede.

use std::{cmp::Ordering, sync::LazyLock};

use regex::Regex;

use crate::anilist::MediaType;

/// Seção da nuvem com correspondência no AniList e profundidade da pasta de título.
///
/// ```text
/// Animes/Letra B/B - The Beginning               → título (profundidade 3)
/// Mangás/Séries Mangás/Letra G/Gantz G (2015)    → título (profundidade 4)
/// ```
struct Section {
    prefix: &'static [&'static str],
    media: MediaType,
    title_depth: usize,
}

const SECTIONS: [Section; 2] = [
    Section { prefix: &["Animes"], media: MediaType::Anime, title_depth: 3 },
    Section { prefix: &["Mangás", "Séries Mangás"], media: MediaType::Manga, title_depth: 4 },
];

#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    Home,
    /// Pasta comum (sem AniList).
    Folder { path: String },
    /// Pasta cujos filhos são títulos → grade de pôsteres.
    Shelf { path: String, media: MediaType },
    /// Dentro de um título. `rest` = subpastas abaixo do título.
    Title { path: String, title_path: String, rest: Vec<String>, media: MediaType },
}

pub fn split_path(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Caminho completo de um item dentro de uma pasta.
pub fn join_path(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") }
}

/// Pasta que contém `path` (`""` na raiz).
pub fn parent_path(path: &str) -> String {
    let segs = split_path(path);
    segs[..segs.len().saturating_sub(1)].join("/")
}

/// Último pedaço do caminho.
pub fn base_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path)
}

pub fn classify(path: &str) -> Place {
    let segs = split_path(path);
    if segs.is_empty() {
        return Place::Home;
    }
    for s in &SECTIONS {
        if s.prefix.len() > segs.len() || s.prefix.iter().zip(&segs).any(|(p, s)| p != s) {
            continue;
        }
        if segs.len() == s.title_depth - 1 {
            return Place::Shelf { path: path.to_owned(), media: s.media };
        }
        if segs.len() >= s.title_depth {
            return Place::Title {
                path: path.to_owned(),
                title_path: segs[..s.title_depth].join("/"),
                rest: segs[s.title_depth..].iter().map(|s| s.to_string()).collect(),
                media: s.media,
            };
        }
    }
    Place::Folder { path: path.to_owned() }
}

/// Tipo de mídia AniList para resultados de busca da nuvem.
pub fn media_for_kind(kind: crate::nuvem::SearchKind) -> Option<MediaType> {
    use crate::nuvem::SearchKind::*;
    match kind {
        Anime => Some(MediaType::Anime),
        Manga => Some(MediaType::Manga),
        _ => None,
    }
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

re!(YEAR, r"[\[(]((?:19|20)\d\d)[\])]");
re!(BRACKETS, r"\[[^\]]*\]|\([^)]*\)");
re!(SPACES, r"\s+");
re!(EDGES, r"^[-_.\s]+|[-_.\s]+$");
re!(TAGS, r"\[([^\]]+)\]");
re!(YEAR_ONLY, r"^(19|20)\d\d$");
re!(EXT, r"(?i)\.[a-z0-9]{2,4}$");
re!(EP_WORD, r"(?i)\b(?:Ep(?:is[oó]dio)?\.?|E)\s*(\d{1,4}(?:\.\d)?)(?:v\d)?\b");
re!(EP_DASH, r"\s-\s(\d{1,4}(?:\.\d)?)(?:v\d)?(?:\s|$|\[)");
re!(QUALITY, r"(?i)\d{3,4}p|BD|WEB|x26[45]|H\.?26[45]");
re!(UNDERSCORES, r"[_.]+");

/// `[2018] B - The Beginning` / `Gantz G (2015)` → 2018 / 2015.
pub fn folder_year(name: &str) -> Option<u32> {
    YEAR.captures(name)?.get(1)?.as_str().parse().ok()
}

/// Remove ano e tags de release: `[2018] B - The Beginning [DUAL]` → `B - The Beginning`.
pub fn folder_title(name: &str) -> String {
    let t = BRACKETS.replace_all(name, " ");
    let t = SPACES.replace_all(&t, " ");
    let t = EDGES.replace_all(t.trim(), "");
    if t.is_empty() { name.to_owned() } else { t.into_owned() }
}

/// Tags de release entre colchetes: `[DUAL] [NF e Troidex] [BD 1080p]`.
pub fn release_tags(name: &str) -> Vec<String> {
    TAGS.captures_iter(name)
        .map(|c| c[1].trim().to_owned())
        .filter(|t| !YEAR_ONLY.is_match(t))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeName {
    pub number: Option<String>,
    pub label: String,
    /// Tags técnicas do arquivo: `BD-Rip 1080p H264 Dual`.
    pub quality: Option<String>,
}

pub fn strip_ext(file_name: &str) -> &str {
    EXT.find(file_name).map_or(file_name, |m| &file_name[..m.start()])
}

/// `[Troidex] B-The Beginning - Ep. 01 [BD-Rip_1080p_H264_Dual].mkv` → `01`.
pub fn parse_episode(file_name: &str) -> EpisodeName {
    let base = strip_ext(file_name);
    // Alguns grupos trocam espaço por `_` (`Show_-_01_[BD]`).
    let spaced = base.replace('_', " ");
    let number = EP_WORD
        .captures(&spaced)
        .or_else(|| EP_DASH.captures(&spaced))
        .map(|c| c[1].to_owned());
    let label = folder_title(base);
    // Último grupo entre colchetes costuma ser a qualidade; o primeiro, o grupo de fansub.
    let groups: Vec<&str> = TAGS.captures_iter(base).map(|c| c.get(1).unwrap().as_str()).collect();
    let raw = if groups.len() > 1 { groups.last().copied() } else { groups.iter().copied().find(|g| QUALITY.is_match(g)) };
    let quality = raw.map(|q| UNDERSCORES.replace_all(q, " ").trim().to_owned());
    EpisodeName { number, label, quality }
}

/// Ordem natural sem diferenciar maiúsculas: `Ep 2` antes de `Ep 10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = fold(x).cmp(&fold(y));
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Letra sem acento e minúscula, para comparar nomes.
fn fold(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'ā' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ē' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'ī' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ō' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'ū' => 'u',
        'ç' => 'c',
        c => c,
    }
}

/// Só letras/números, minúsculo, sem acento (comparar títulos).
pub fn norm(s: &str) -> String {
    s.chars().map(fold).filter(|c| c.is_ascii_alphanumeric()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_places() {
        assert_eq!(classify(""), Place::Home);
        assert_eq!(classify("Animes/Letra B"), Place::Shelf { path: "Animes/Letra B".into(), media: MediaType::Anime });
        assert_eq!(
            classify("Animes/Letra B/B - The Beginning/Extras"),
            Place::Title {
                path: "Animes/Letra B/B - The Beginning/Extras".into(),
                title_path: "Animes/Letra B/B - The Beginning".into(),
                rest: vec!["Extras".into()],
                media: MediaType::Anime
            }
        );
        assert!(matches!(classify("Mangás/Séries Mangás/Letra G"), Place::Shelf { media: MediaType::Manga, .. }));
        assert_eq!(classify("Doramas/X"), Place::Folder { path: "Doramas/X".into() });
        assert_eq!(classify("Animes"), Place::Folder { path: "Animes".into() });
    }

    #[test]
    fn folder_names() {
        assert_eq!(folder_year("[2018] B - The Beginning"), Some(2018));
        assert_eq!(folder_year("Gantz G (2015)"), Some(2015));
        assert_eq!(folder_year("Sem ano"), None);
        assert_eq!(folder_title("[2018] B - The Beginning [DUAL]"), "B - The Beginning");
        assert_eq!(folder_title("[2018]"), "[2018]");
        assert_eq!(release_tags("[2018] X [DUAL] [NF e Troidex]"), vec!["DUAL", "NF e Troidex"]);
    }

    #[test]
    fn episode_names() {
        let e = parse_episode("[Troidex] B-The Beginning - Ep. 01 [BD-Rip_1080p_H264_Dual].mkv");
        assert_eq!(e.number.as_deref(), Some("01"));
        assert_eq!(e.quality.as_deref(), Some("BD-Rip 1080p H264 Dual"));
        assert_eq!(e.label, "B-The Beginning - Ep. 01");
        assert_eq!(parse_episode("Show - 12 [1080p].mkv").number.as_deref(), Some("12"));
        assert_eq!(parse_episode("Show Episódio 3.mp4").number.as_deref(), Some("3"));
        assert_eq!(parse_episode("[G] Filme [1080p].mkv").quality.as_deref(), Some("1080p"));
        let u = parse_episode("[#AnimeNSK]_Caligula_-_01_[BD_1920x1080]_[HEVC_10Bit]_(FB10B460).mkv");
        assert_eq!(u.number.as_deref(), Some("01"));
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["Ep 10.mkv", "ep 2.mkv", "Ep 1.mkv", "Ábaco", "abacaxi"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["abacaxi", "Ábaco", "Ep 1.mkv", "ep 2.mkv", "Ep 10.mkv"]);
    }

    #[test]
    fn paths() {
        assert_eq!(join_path("", "a"), "a");
        assert_eq!(join_path("a/b", "c"), "a/b/c");
        assert_eq!(parent_path("a/b/c"), "a/b");
        assert_eq!(parent_path("a"), "");
        assert_eq!(base_name("a/b/c"), "c");
        assert_eq!(norm("Kimetsu no Yaiba: Mugen-Ressha!"), "kimetsunoyaibamugenressha");
    }
}
