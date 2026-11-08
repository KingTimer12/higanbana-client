//! Filtros e ordenação do catálogo de Animes (lógica pura, sem rede).

use std::sync::LazyLock;

use regex::Regex;

use crate::{
    anilist::types::MediaSummary,
    library::{WorkKind, norm},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

impl Season {
    pub fn from_anilist(s: &str) -> Option<Season> {
        match s {
            "WINTER" => Some(Season::Winter),
            "SPRING" => Some(Season::Spring),
            "SUMMER" => Some(Season::Summer),
            "FALL" => Some(Season::Fall),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Filters {
    pub query: String,
    pub kinds: Vec<WorkKind>,
    pub genres: Vec<String>,
    pub years: Option<(i32, i32)>,
    pub seasons: Vec<Season>,
    pub dubbed: bool,
}

impl Filters {
    /// Algum filtro além de `query`/`dubbed`, isto é, que só os metadados respondem.
    pub fn needs_metadata(&self) -> bool {
        !self.kinds.is_empty() || !self.genres.is_empty() || self.years.is_some() || !self.seasons.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.query.is_empty() && !self.dubbed && !self.needs_metadata()
    }

    fn matches(&self, m: &MediaSummary) -> bool {
        if !self.kinds.is_empty() && !m.format.as_deref().and_then(WorkKind::from_anilist).is_some_and(|k| self.kinds.contains(&k)) {
            return false;
        }
        if !self.genres.iter().all(|g| m.genres.iter().any(|x| x.eq_ignore_ascii_case(g))) {
            return false;
        }
        if let Some((from, to)) = self.years
            && !m.year().is_some_and(|y| (from..=to).contains(&y))
        {
            return false;
        }
        self.seasons.is_empty() || m.season.as_deref().and_then(Season::from_anilist).is_some_and(|s| self.seasons.contains(&s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Az,
    Year,
    Score,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Unknown,
}

/// Decide se uma pasta de obra entra no catálogo. `inner` são os itens já
/// conhecidos de uma franquia aberta: basta um deles passar.
pub fn judge(name: &str, meta: Option<&MediaSummary>, inner: &[MediaSummary], f: &Filters) -> Verdict {
    if !f.query.is_empty() && !norm(name).contains(&norm(&f.query)) {
        return Verdict::Fail;
    }
    if f.dubbed && !is_dubbed(name) {
        return Verdict::Fail;
    }
    if !f.needs_metadata() {
        return Verdict::Pass;
    }
    if meta.is_none() && inner.is_empty() {
        return Verdict::Unknown;
    }
    if meta.into_iter().chain(inner).any(|m| f.matches(m)) { Verdict::Pass } else { Verdict::Fail }
}

/// Etiqueta de áudio: `DUAL`, `Dublado` ou `Multi-Audio` entre `[]`/`()`; as duas
/// últimas também como palavra solta. `Dual` solto não conta (há títulos como
/// "Dual Parallel Trouble Adventures").
pub fn is_dubbed(name: &str) -> bool {
    static TAG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)[\[(]\s*(?:dual|dublado|multi-audio)\s*[\])]|(?:^|[^[:alnum:]])(?:dublado|multi-audio)(?:[^[:alnum:]]|$)").unwrap()
    });
    TAG.is_match(name)
}

/// Chave para ordenar do mais novo ao mais antigo; sem ano vai para o fim.
pub fn sort_key_year(meta: Option<&MediaSummary>) -> i32 {
    meta.and_then(MediaSummary::year).unwrap_or(i32::MIN)
}

/// Chave para ordenar da maior nota à menor; sem nota vai para o fim.
pub fn sort_key_score(meta: Option<&MediaSummary>) -> i64 {
    meta.and_then(|m| m.average_score).unwrap_or(i64::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn summary(v: serde_json::Value) -> MediaSummary {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn judges_entries() {
        let tv2010 = summary(json!({"id":1,"format":"TV","seasonYear":2010,"season":"WINTER","genres":["Mahou Shoujo","Comedy"]}));
        let f = Filters { genres: vec!["Mahou Shoujo".into()], kinds: vec![WorkKind::Tv], ..Default::default() };
        assert_eq!(judge("Heartcatch Precure!", Some(&tv2010), &[], &f), Verdict::Pass);
        assert_eq!(judge("X", None, &[], &f), Verdict::Unknown);
        let movie = summary(json!({"id":2,"format":"MOVIE","seasonYear":2005,"genres":["Action"]}));
        assert_eq!(judge("Precure", Some(&movie), &[tv2010.clone()], &f), Verdict::Pass);
        assert_eq!(judge("Precure", Some(&movie), &[], &f), Verdict::Fail);
        let q = Filters { query: "heart catch".into(), ..Default::default() };
        assert_eq!(judge("[2010] Heartcatch Precure!", None, &[], &q), Verdict::Pass);
        let y = Filters { years: Some((2009, 2011)), seasons: vec![Season::Winter], ..Default::default() };
        assert_eq!(judge("x", Some(&tv2010), &[], &y), Verdict::Pass);
        assert!(is_dubbed("Naruto [DUAL]") && !is_dubbed("Naruto"));
        let two = Filters { genres: vec!["Mahou Shoujo".into(), "Action".into()], ..Default::default() };
        assert_eq!(judge("x", Some(&tv2010), &[], &two), Verdict::Fail);
        let late = Filters { years: Some((2015, 2020)), ..Default::default() };
        assert_eq!(judge("x", Some(&tv2010), &[], &late), Verdict::Fail);
        let summer = Filters { seasons: vec![Season::Summer], ..Default::default() };
        assert_eq!(judge("x", Some(&tv2010), &[], &summer), Verdict::Fail);
        assert!(!Filters::default().needs_metadata());
        assert!(f.needs_metadata());
    }

    #[test]
    fn dubbed_only_by_tag() {
        assert!(!is_dubbed("Dual Parallel Trouble Adventures"));
        assert!(is_dubbed("Naruto [DUAL]"));
        assert!(is_dubbed("X [Multi-Audio] [1080p]"));
        assert!(is_dubbed("[DragsterPS] Glitter Force S01 [1080p] [Multi-Audio]"));
        assert!(is_dubbed("Bleach (Dublado)"));
        assert!(is_dubbed("Bleach Dublado 720p"));
        assert!(!is_dubbed("Dualist"));
    }
}
