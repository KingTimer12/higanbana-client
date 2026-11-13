//! Formatos de pasta reconhecidos dentro da pasta base de uma obra.
//!
//! Uma regra recebe a pasta base, as subpastas dela e a obra do AniList, e
//! devolve candidatas. A mesma lista vale para anime e mangá: um formato novo
//! entra aqui uma vez e passa a valer nas duas seções.

use crate::{
    anilist::types::MediaSummary,
    library::{WorkKind, folder_title, is_franchise, join_path, norm, parse_work_name},
};

/// Candidata a pasta da obra.
#[derive(Debug, Clone, PartialEq)]
pub enum Candidate {
    /// Casou sem dúvida (nome igual a um título da obra).
    Path(String),
    /// Casou por ano e formato; o nome da pasta precisa resolver no AniList para o mesmo id.
    Confirm { name: String, path: String },
}

pub type Rule = fn(base_path: &str, dirs: &[String], m: &MediaSummary) -> Vec<Candidate>;

/// Regras consultadas em ordem.
pub const RULES: &[Rule] = &[dated_franchise];

/// Títulos da obra com letras latinas (romaji, inglês, preferido e sinônimos).
pub fn titles_of(m: &MediaSummary) -> Vec<String> {
    let t = &m.title;
    [t.romaji.clone(), t.english.clone(), t.user_preferred.clone()]
        .into_iter()
        .flatten()
        .chain(m.synonyms.iter().cloned())
        .filter(|s| s.chars().any(|c| c.is_ascii_alphabetic()))
        .collect()
}

/// Franquia com subpastas datadas: `[2017] Título Season 2 [TV]`.
///
/// Nome igual a um título → [`Candidate::Path`]. Mesmo ano e formato compatível
/// (pasta sem marca de formato conta como compatível) → [`Candidate::Confirm`].
pub fn dated_franchise(base_path: &str, dirs: &[String], m: &MediaSummary) -> Vec<Candidate> {
    let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
    if !is_franchise(&refs) {
        return Vec::new();
    }
    let titles: Vec<String> = titles_of(m).iter().map(|t| norm(t)).collect();
    let year = m.season_year.or(m.start_date.as_ref().and_then(|d| d.year));
    let kind = m.format.as_deref().and_then(WorkKind::from_anilist);
    let (mut exact, mut confirm) = (Vec::new(), Vec::new());
    for name in dirs {
        let path = join_path(base_path, name);
        if titles.contains(&norm(&folder_title(name))) {
            exact.push(Candidate::Path(path));
            continue;
        }
        let w = parse_work_name(name);
        let same_year = year.is_some() && w.year.map(|y| y as i32) == year;
        let same_kind = w.kind.is_none() || kind.is_none() || w.kind == kind;
        if same_year && same_kind {
            confirm.push(Candidate::Confirm { name: name.clone(), path });
        }
    }
    exact.extend(confirm);
    exact
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(id: i64, title: &str, year: i32, format: &str) -> MediaSummary {
        serde_json::from_value(serde_json::json!({
            "id": id, "type": "ANIME", "format": format,
            "title": { "romaji": title }, "seasonYear": year,
        }))
        .unwrap()
    }

    fn dirs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    const SNK: [&str; 2] = ["[2013] Shingeki no Kyojin [TV]", "[2017] Shingeki no Kyojin Season 2 [TV]"];

    #[test]
    fn franchise_exact_title() {
        let m = media(1, "Shingeki no Kyojin Season 2", 2017, "TV");
        assert_eq!(dated_franchise("base", &dirs(&SNK), &m), vec![Candidate::Path("base/[2017] Shingeki no Kyojin Season 2 [TV]".into())]);
    }

    #[test]
    fn franchise_year_and_kind_needs_confirm() {
        let m = media(2, "Attack on Titan S2", 2017, "TV");
        assert_eq!(
            dated_franchise("base", &dirs(&SNK), &m),
            vec![Candidate::Confirm {
                name: "[2017] Shingeki no Kyojin Season 2 [TV]".into(),
                path: "base/[2017] Shingeki no Kyojin Season 2 [TV]".into()
            }]
        );
    }

    #[test]
    fn same_year_other_kind_ignored() {
        let d = dirs(&["[2013] X [TV]", "[2019] X [FILME]"]);
        assert_eq!(dated_franchise("base", &d, &media(3, "X Season 3", 2019, "TV")), vec![]);
    }

    #[test]
    fn missing_season_is_empty() {
        assert_eq!(dated_franchise("base", &dirs(&SNK), &media(4, "Shingeki no Kyojin Season 3", 2019, "TV")), vec![]);
    }

    #[test]
    fn not_franchise_is_empty() {
        let d = dirs(&["1080p BD", "Extras"]);
        assert_eq!(dated_franchise("base", &d, &media(5, "1080p BD", 2019, "TV")), vec![]);
    }
}
