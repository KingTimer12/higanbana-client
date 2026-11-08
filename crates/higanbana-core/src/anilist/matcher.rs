//! Casamento entre nomes de pasta da nuvem e resultados do AniList.
//!
//! Exemplos de pastas: `B - The Beginning`, `[2018] B - The Beginning`,
//! `Gantz G (2015)`, `Nagao Takena [Curtas]`,
//! `B - The Beginning [DUAL] [NF e Troidex] [BD 1080p]`.

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    pub title: String,
    pub year: Option<i32>,
}

/// Extrai título limpo e ano de um nome de pasta.
pub fn parse_folder_name(name: &str) -> ParsedName {
    let mut year = None;
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();

    while let Some(c) = chars.next() {
        let close = match c {
            '[' => ']',
            '(' => ')',
            _ => {
                out.push(c);
                continue;
            }
        };
        let mut inner = String::new();
        let mut closed = false;
        for d in chars.by_ref() {
            if d == close {
                closed = true;
                break;
            }
            inner.push(d);
        }
        if !closed {
            out.push(c);
            out.push_str(&inner);
            continue;
        }
        let inner = inner.trim();
        // `[2005.1]` (ano + ordem de lançamento no mesmo ano) vale como 2005.
        let year_text = inner.split('.').next().unwrap_or_default();
        if year.is_none()
            && year_text.len() == 4
            && let Ok(y) = year_text.parse::<i32>()
            && (1900..=2100).contains(&y)
        {
            year = Some(y);
        }
        // Qualquer outro grupo entre colchetes/parênteses é tag de release; descarta.
        out.push(' ');
    }

    let title = out
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '-' || c == '_' || c == '.' || c.is_whitespace())
        .to_owned();

    ParsedName {
        title: if title.is_empty() { name.trim().to_owned() } else { title },
        year,
    }
}

/// Minúsculas, só letras/números (inclui CJK), `×` vira `x`.
pub fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c == '×' { 'x' } else { c })
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Coeficiente de Dice sobre bigramas de caracteres (0..=1).
pub fn similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (normalize(a), normalize(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let bigrams = |s: &str| -> Vec<(char, char)> {
        let v: Vec<char> = s.chars().collect();
        if v.len() < 2 {
            return vec![(v[0], ' ')];
        }
        v.windows(2).map(|w| (w[0], w[1])).collect()
    };
    let (ba, mut bb) = (bigrams(&a), bigrams(&b));
    let total = ba.len() + bb.len();
    let mut hits = 0;
    for g in &ba {
        if let Some(pos) = bb.iter().position(|x| x == g) {
            bb.swap_remove(pos);
            hits += 1;
        }
    }
    (2 * hits) as f64 / total as f64
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    #[serde(default)]
    title: Titles,
    #[serde(default)]
    synonyms: Vec<String>,
    season_year: Option<i32>,
    start_date: Option<FuzzyDate>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Titles {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FuzzyDate {
    year: Option<i32>,
}

const MIN_SIMILARITY: f64 = 0.5;

/// Escolhe o melhor candidato (lista já na ordem `SEARCH_MATCH` do AniList).
pub fn pick_best(candidates: &[Value], wanted: &ParsedName) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;

    for (rank, raw) in candidates.iter().enumerate() {
        let Ok(c) = serde_json::from_value::<Candidate>(raw.clone()) else {
            continue;
        };
        let sim = [&c.title.romaji, &c.title.english, &c.title.native]
            .into_iter()
            .flatten()
            .chain(c.synonyms.iter())
            .map(|t| similarity(t, &wanted.title))
            .fold(0.0, f64::max);
        if sim < MIN_SIMILARITY {
            continue;
        }

        let cand_year = c.season_year.or(c.start_date.and_then(|d| d.year));
        let year_score = match (wanted.year, cand_year) {
            (Some(w), Some(y)) if w == y => 0.4,
            (Some(w), Some(y)) => -0.1 * (w - y).abs().min(4) as f64,
            _ => 0.0,
        };
        // Leve preferência pela ordem do AniList.
        let score = sim + year_score - 0.02 * rank as f64;

        if best.is_none_or(|(_, s)| score > s) {
            best = Some((rank, score));
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(title: &str, year: Option<i32>) -> ParsedName {
        ParsedName { title: title.into(), year }
    }

    #[test]
    fn parses_folder_names() {
        assert_eq!(parse_folder_name("B - The Beginning"), p("B - The Beginning", None));
        assert_eq!(parse_folder_name("[2018] B - The Beginning"), p("B - The Beginning", Some(2018)));
        assert_eq!(parse_folder_name("Gantz G (2015)"), p("Gantz G", Some(2015)));
        assert_eq!(parse_folder_name("Nagao Takena [Curtas]"), p("Nagao Takena", None));
        assert_eq!(
            parse_folder_name("B - The Beginning [DUAL] [NF e Troidex] [BD 1080p]"),
            p("B - The Beginning", None)
        );
        assert_eq!(parse_folder_name("\"Eiji\""), p("\"Eiji\"", None));
        assert_eq!(parse_folder_name("[Colecao Curtas]"), p("[Colecao Curtas]", None));
        assert_eq!(
            parse_folder_name("[2005.1] Futari wa Precure Max Heart Movie 1 [FILME]"),
            p("Futari wa Precure Max Heart Movie 1", Some(2005))
        );
    }

    #[test]
    fn similarity_ignores_punctuation() {
        assert_eq!(similarity("B - The Beginning", "B: The Beginning"), 1.0);
        assert_eq!(similarity("86 Eighty-Six", "86: Eighty Six"), 1.0);
        assert_eq!(similarity("91 Days", "91Days"), 1.0);
        assert_eq!(similarity("3x3 Eyes", "3×3 EYES"), 1.0);
        assert!(similarity("Howl no Ugoku Shiro", "Hyouge Mono") < 0.5);
    }

    fn media(romaji: &str, year: i32) -> Value {
        json!({ "title": { "romaji": romaji }, "synonyms": [], "seasonYear": year })
    }

    #[test]
    fn prefers_matching_year() {
        let c = vec![media("86: Eighty Six", 2021), media("86: Eighty Six", 2022)];
        assert_eq!(pick_best(&c, &p("86 Eighty-Six", Some(2022))), Some(1));
        assert_eq!(pick_best(&c, &p("86 Eighty-Six", None)), Some(0));
    }

    #[test]
    fn rejects_unrelated() {
        let c = vec![media("Hyouge Mono", 2011)];
        assert_eq!(pick_best(&c, &p("Howl no Ugoku Shiro", None)), None);
    }

    #[test]
    fn matches_synonyms() {
        let c = vec![json!({ "title": { "romaji": "Hauru no Ugoku Shiro" }, "synonyms": ["Howl no Ugoku Shiro"], "seasonYear": 2004 })];
        assert_eq!(pick_best(&c, &p("Howl no Ugoku Shiro", None)), Some(0));
    }
}
