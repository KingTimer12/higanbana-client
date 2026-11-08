//! Como o acervo da nuvem se organiza: seções, pastas de título, nomes de
//! episódio e ordem natural. Tudo puro, sem rede.

use std::{cmp::Ordering, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{anilist::MediaType, anime_index::LETTERS};

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
    /// Catálogo de Animes (A–Z contínuo); `letter` abre já rolado até ela.
    Catalog { letter: Option<String> },
    /// Pasta aberta como está, sem catálogo nem prateleira (ver [`raw_path`]).
    Raw { path: String },
}

const RAW_PREFIX: &str = "raw:";

/// Rota que abre `path` como pasta comum.
pub fn raw_path(path: &str) -> String {
    format!("{RAW_PREFIX}{path}")
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
    if let Some(raw) = path.strip_prefix(RAW_PREFIX) {
        return Place::Raw { path: raw.to_owned() };
    }
    let segs = split_path(path);
    if segs.is_empty() {
        return Place::Home;
    }
    // Animes e suas letras formam um catálogo só; coleções seguem como prateleira.
    match segs[..] {
        ["Animes"] => return Place::Catalog { letter: None },
        ["Animes", letter] if LETTERS.contains(&letter) => return Place::Catalog { letter: Some(letter.to_owned()) },
        _ => {}
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

re!(YEAR, r"[\[(]((?:19|20)\d\d)(?:\.(\d+))?[\])]");
re!(DATED, r"^\s*\[(?:19|20)\d\d(?:\.\d+)?\]");
re!(KIND_TAG, r"(?i)\[\s*(TV|FILME|MOVIE|OVA|ONA|SPECIAL|ESPECIAL)\s*\]");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkKind {
    Tv,
    Movie,
    Ova,
    Ona,
    Special,
}

impl WorkKind {
    /// Formato do AniList (`TV`, `MOVIE`, ...) → tipo de obra.
    pub fn from_anilist(format: &str) -> Option<WorkKind> {
        match format {
            "TV" | "TV_SHORT" => Some(WorkKind::Tv),
            "MOVIE" => Some(WorkKind::Movie),
            "OVA" => Some(WorkKind::Ova),
            "ONA" => Some(WorkKind::Ona),
            "SPECIAL" | "MUSIC" => Some(WorkKind::Special),
            _ => None,
        }
    }
}

/// Pasta de obra dentro de uma franquia: `[2005.1] Título [FILME]`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkName {
    pub year: Option<u32>,
    /// Parte decimal do ano (`2005.1` → 1); 0 quando não há.
    pub order: u32,
    pub title: String,
    pub kind: Option<WorkKind>,
}

pub fn parse_work_name(name: &str) -> WorkName {
    let caps = YEAR.captures(name);
    let year = caps.as_ref().and_then(|c| c.get(1)?.as_str().parse().ok());
    let order = caps.as_ref().and_then(|c| c.get(2)?.as_str().parse().ok()).unwrap_or(0);
    let kind = KIND_TAG.captures(name).and_then(|c| match c[1].to_ascii_uppercase().as_str() {
        "TV" => Some(WorkKind::Tv),
        "FILME" | "MOVIE" => Some(WorkKind::Movie),
        "OVA" => Some(WorkKind::Ova),
        "ONA" => Some(WorkKind::Ona),
        "SPECIAL" | "ESPECIAL" => Some(WorkKind::Special),
        _ => None,
    });
    WorkName { year, order, title: folder_title(name), kind }
}

/// Começa com `[AAAA]` ou `[AAAA.N]`.
pub fn is_dated(name: &str) -> bool {
    DATED.is_match(name)
}

/// Pelo menos 2 subpastas e mais da metade com ano na frente.
pub fn is_franchise(dirs: &[&str]) -> bool {
    dirs.len() >= 2 && dirs.iter().filter(|d| is_dated(d)).count() * 2 > dirs.len()
}

/// Remove ano e tags de release: `[2018] B - The Beginning [DUAL]` → `B - The Beginning`.
pub fn folder_title(name: &str) -> String {
    let t = BRACKETS.replace_all(name, " ");
    let t = SPACES.replace_all(&t, " ");
    let t = EDGES.replace_all(t.trim(), "");
    if t.is_empty() { name.to_owned() } else { t.into_owned() }
}

re!(LEADING_GROUP, r"^\s*\[([^\]]+)\]");
// `-Grupo` no fim: depois de marca técnica (`H.264-TupaC`) ou em nome sem espaços com pontos.
re!(
    TRAILING_GROUP_TECH,
    r"(?i)(?:\b\d{3,4}p|\bx26[45]|\bH\.?26[45]|\bAAC\d*(?:\.\d)?|\b(?:AVC|HEVC|FLAC|DL|BD|CR|NF|AMZN|Dual))-([A-Za-z0-9]+)$"
);
re!(TRAILING_GROUP_SCENE, r"^\S*\.\S*-([A-Za-z0-9]+)$");
re!(SRC_CR, r"(?i)(?:^|[^A-Za-z0-9])CR(?:[^A-Za-z0-9]|$)");
re!(SRC_NF, r"(?i)(?:^|[^A-Za-z0-9])NF(?:[^A-Za-z0-9]|$)");
re!(SRC_AMZN, r"(?i)(?:^|[^A-Za-z0-9])AMZN(?:[^A-Za-z0-9]|$)");
re!(SRC_BD, r"(?i)(?:^|[^A-Za-z0-9])BD(?:[^A-Za-z0-9]|$)");
re!(SRC_WEB_DL, r"(?i)(?:^|[^A-Za-z0-9])WEB-DL(?:[^A-Za-z0-9]|$)");
re!(SRC_WEB, r"(?i)(?:^|[^A-Za-z0-9])WEB(?:[^A-Za-z0-9]|$)");
re!(RES_P, r"(?i)\b(\d{3,4})p\b");
re!(RES_WXH, r"\b\d{3,4}x(\d{3,4})\b");
re!(AUDIO, r"(?i)\b(Multi-Audio|Dual|Dublado|Dub)\b");
re!(
    TECH,
    r"(?i)\b(?:e\s+)?S\d{1,2}\b|\b\d{3,4}p\b|\b\d{3,4}x\d{3,4}\b|\bx26[45]\b|\bH\.?26[45]\b|\b(?:AVC|HEVC|FLAC|hi10p?)\b|\bAAC\d*(?:\.\d)?|\bWEB-DL\b|\b(?:WEB|BD|CR|NF|AMZN)\b|\b\d+\s*~\s*\d+\b"
);

pub const LOOSE_FILES_VERSION: &str = "Arquivos da pasta";

/// Rótulo legível de uma pasta de versão (release) de uma obra.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VersionLabel {
    pub name: String,
    pub group: Option<String>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub audio: Option<String>,
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).map(norm).filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 3).collect()
}

/// Pasta sem colchetes e sem `_`, sobre a qual o grupo final é detectado e removido.
fn trailing_group(cleaned: &str) -> Option<(usize, String)> {
    let c = TRAILING_GROUP_TECH.captures(cleaned).or_else(|| TRAILING_GROUP_SCENE.captures(cleaned))?;
    let g = c.get(1)?;
    (!g.as_str().eq_ignore_ascii_case("DL") && !g.as_str().eq_ignore_ascii_case("Audio"))
        .then(|| (g.start() - 1, g.as_str().to_owned()))
}

pub fn version_label(folder: &str, work_title: &str) -> VersionLabel {
    let mut base = BRACKETS.replace_all(folder, " ").replace('_', " ").trim().to_owned();
    let leading = LEADING_GROUP.captures(folder).map(|c| c[1].trim().to_owned());
    let trailing = if leading.is_none() { trailing_group(&base) } else { None };
    let group = leading.or_else(|| trailing.as_ref().map(|t| t.1.clone()));
    let resolution = RES_P
        .captures(folder)
        .map(|c| format!("{}p", &c[1]))
        .or_else(|| RES_WXH.captures(folder).map(|c| format!("{}p", &c[1])));
    let sources: [(&str, &Regex); 6] = [
        ("CR", &SRC_CR),
        ("NF", &SRC_NF),
        ("AMZN", &SRC_AMZN),
        ("BD", &SRC_BD),
        ("WEB-DL", &SRC_WEB_DL),
        ("WEB", &SRC_WEB),
    ];
    let source = sources.into_iter().find(|(_, r)| r.is_match(folder)).map(|(n, _)| n);
    let audio = AUDIO.captures(folder).map(|c| match c[1].to_ascii_lowercase().as_str() {
        "multi-audio" => "Multi-Audio",
        "dual" => "Dual",
        "dublado" => "Dublado",
        _ => "Dub",
    });

    // Nome sem colchetes nem marcas técnicas.
    if let Some((at, _)) = trailing {
        base.truncate(at);
    }
    let base = TECH.replace_all(&base, " ").replace('.', " ");
    let base = SPACES.replace_all(&base, " ");
    let title = EDGES.replace_all(base.trim(), "").into_owned();

    // Título próprio (ex.: "Glitter Force" para Smile Precure) só quando não tem relação com a obra.
    let (nt, nw) = (norm(&title), norm(work_title));
    let work_words = words(work_title);
    let own_title = nt.chars().filter(|c| c.is_alphabetic()).count() >= 3
        && !nw.contains(&nt)
        && !nt.contains(&nw)
        && !words(&title).iter().any(|w| work_words.contains(w));
    let name = if own_title {
        title
    } else if let Some(g) = &group {
        g.clone()
    } else if !title.is_empty() {
        title
    } else {
        folder_title(folder)
    };
    VersionLabel { name, group, resolution, source: source.map(str::to_owned), audio: audio.map(str::to_owned) }
}

/// Altura em pixels (`1080p` → 1080); 0 sem resolução.
pub fn resolution_rank(label: &VersionLabel) -> u32 {
    label.resolution.as_deref().and_then(|r| r.trim_end_matches('p').parse().ok()).unwrap_or(0)
}

/// `Extras`, `Especiais`, `Bônus`, `Legendas`, `NCOP`, `NCED`, `OST`, `Scans` (ou começando por elas).
pub fn is_extra_folder(name: &str) -> bool {
    const EXTRAS: [&str; 9] = ["extras", "especiais", "bonus", "legendas", "ncop", "nced", "ost", "scans", "extra"];
    name.split(|c: char| !c.is_alphanumeric()).find(|w| !w.is_empty()).is_some_and(|w| EXTRAS.contains(&norm(w).as_str()))
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

    #[test]
    fn version_label_group_and_title_are_consistent() {
        let v = version_label("Re-Zero [1080p]", "Something Else");
        assert_eq!(v.group, None);
        assert!(v.name.contains("Re-Zero") || v.name.contains("Re Zero"), "{}", v.name);
        let v = version_label("[Grp] Spider-Man [1080p]", "Other");
        assert!(v.name.contains("Spider") && v.name.contains("Man"), "{}", v.name);
        let v = version_label("Show Name WEB-DL x264-Grp", "Another");
        assert_eq!(v.group.as_deref(), Some("Grp"));
        assert_eq!(v.name, "Show Name");
        // Sem grupo e com título relacionado à obra: usa o título limpo.
        let v = version_label("Smile Precure 1080p", "Smile Precure!");
        assert_eq!((v.group, v.name.as_str()), (None, "Smile Precure"));
        // Sem grupo nem texto útil: cai em folder_title.
        assert_eq!(version_label("(1080p)", "x").name, "(1080p)");
    }

    #[test]
    fn version_labels() {
        let v = version_label("[Aenianos]_Pretty_Cure_Splash_Star [640x480]", "Futari wa Precure Splash Star");
        assert_eq!((v.group.as_deref(), v.resolution.as_deref()), (Some("Aenianos"), Some("480p")));
        assert_eq!(v.name, "Aenianos");
        let v = version_label("PreCure.Splash.Star.S01.1080p.CR.WEB-DL.AAC2.0.H.264-TupaC", "Futari wa Precure Splash Star");
        assert_eq!((v.group.as_deref(), v.resolution.as_deref(), v.source.as_deref()), (Some("TupaC"), Some("1080p"), Some("CR")));
        let v = version_label("[DragsterPS] Glitter Force S01 e S02 [1080p] [Multi-Audio] [Multi-Subs]", "Smile Precure!");
        assert_eq!(v.name, "Glitter Force");
        assert_eq!((v.audio.as_deref(), v.resolution.as_deref()), (Some("Multi-Audio"), Some("1080p")));
        let v = version_label("[Aenianos] Smile Precure [1280x720][hi10p]", "Smile Precure!");
        assert_eq!((v.name.as_str(), v.resolution.as_deref()), ("Aenianos", Some("720p")));
        assert_eq!(version_label("Yes Precure 5 - 01 ~ 49 [1080p CR WEB-DL AVC AAC][MultiSub]", "Yes! Precure 5").source.as_deref(), Some("CR"));
        assert!(resolution_rank(&version_label("x [1080p]", "x")) > resolution_rank(&version_label("x [640x480]", "x")));
        assert!(is_extra_folder("Extras") && is_extra_folder("NCOP e NCED") && is_extra_folder("bônus"));
        assert!(!is_extra_folder("[Aenianos] Smile Precure"));
    }
    use super::*;

    #[test]
    fn classifies_places() {
        assert_eq!(classify(""), Place::Home);
        assert_eq!(classify("Animes/Letra B"), Place::Catalog { letter: Some("Letra B".into()) });
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
        assert_eq!(classify("Animes"), Place::Catalog { letter: None });
    }

    #[test]
    fn classifies_catalog() {
        assert_eq!(classify("Animes"), Place::Catalog { letter: None });
        assert_eq!(classify("Animes/Letra P"), Place::Catalog { letter: Some("Letra P".into()) });
        assert_eq!(classify("Animes/0-9"), Place::Catalog { letter: Some("0-9".into()) });
        assert_eq!(
            classify("Animes/[Colecao Curtas]"),
            Place::Shelf { path: "Animes/[Colecao Curtas]".into(), media: MediaType::Anime }
        );
        assert!(matches!(classify("Animes/Letra P/Precure"), Place::Title { media: MediaType::Anime, .. }));
        assert_eq!(raw_path("Animes/Letra P"), "raw:Animes/Letra P");
        assert_eq!(classify(&raw_path("Animes/Letra P")), Place::Raw { path: "Animes/Letra P".into() });
        assert_eq!(classify("raw:Animes"), Place::Raw { path: "Animes".into() });
        // Mangás continuam em prateleira.
        assert!(matches!(classify("Mangás/Séries Mangás/Letra P"), Place::Shelf { media: MediaType::Manga, .. }));
    }

    #[test]
    fn work_names() {
        let w = parse_work_name("[2005.1] Futari wa Precure Max Heart Movie 1 [FILME]");
        assert_eq!((w.year, w.order, w.kind), (Some(2005), 1, Some(WorkKind::Movie)));
        assert_eq!(w.title, "Futari wa Precure Max Heart Movie 1");
        let w = parse_work_name("[2004] Futari wa Precure  [TV]");
        assert_eq!((w.year, w.order, w.kind, w.title.as_str()), (Some(2004), 0, Some(WorkKind::Tv), "Futari wa Precure"));
        assert_eq!(parse_work_name("[2010.1] Heartcatch Precure! Movie Hana no Miyako [FILME] ").kind, Some(WorkKind::Movie));
        assert_eq!(parse_work_name("[2008.3] Precure All Stars GoGo Dream Live! [SPECIAL]").kind, Some(WorkKind::Special));
        assert_eq!(parse_work_name("[2011.3] X [OVA]").kind, Some(WorkKind::Ova));
        assert_eq!(parse_work_name("[2011.3] X [ona]").kind, Some(WorkKind::Ona));
        assert_eq!(parse_work_name("[2011.3] X [Especial]").kind, Some(WorkKind::Special));
        assert_eq!(parse_work_name("[2026] Meitantei Precure!").kind, None);
        assert_eq!(parse_work_name("Sem ano").year, None);
        assert_eq!(folder_year("[2005.1] X"), Some(2005));
        assert!(is_dated("[2005.0] X") && is_dated("[2004] X") && !is_dated("X (2004)"));
        assert!(is_franchise(&["[2004] A", "[2005.0] B", "Extras"]));
        assert!(!is_franchise(&["[2004] A"]));
        assert!(!is_franchise(&["[Aenianos] A", "B 1080p"]));
        assert_eq!(WorkKind::from_anilist("TV_SHORT"), Some(WorkKind::Tv));
        assert_eq!(WorkKind::from_anilist("MUSIC"), Some(WorkKind::Special));
        assert_eq!(WorkKind::from_anilist("X"), None);
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
