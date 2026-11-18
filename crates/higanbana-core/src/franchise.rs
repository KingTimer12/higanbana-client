//! Abas de uma franquia: pasta cujas subpastas são obras datadas
//! (`[2004] Futari wa Precure [TV]`, `[2005.1] Movie 1 [FILME]`).
//! O desktop e o app mobile agrupam as obras do mesmo jeito.

use crate::library::{WorkKind, WorkName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FranchiseTab {
    Tv,
    Movies,
    Specials,
    Others,
}

impl FranchiseTab {
    /// Aba de uma obra. Sem ano → Outros; com ano, o formato do AniList/MAL
    /// (`format`) vence a tag do nome, e obra sem formato conhecido conta como série.
    pub fn of(name: &WorkName, format: Option<&str>) -> Self {
        if name.year.is_none() {
            return FranchiseTab::Others;
        }
        match format.and_then(WorkKind::from_anilist).or(name.kind) {
            Some(WorkKind::Movie) => FranchiseTab::Movies,
            Some(WorkKind::Ova | WorkKind::Special) => FranchiseTab::Specials,
            Some(WorkKind::Tv | WorkKind::Ona) | None => FranchiseTab::Tv,
        }
    }

    /// `Séries TV · 3`; Outros vai sem a contagem.
    pub fn label(self, count: usize) -> String {
        match self {
            FranchiseTab::Tv => format!("Séries TV · {count}"),
            FranchiseTab::Movies => format!("Filmes · {count}"),
            FranchiseTab::Specials => format!("Especiais · {count}"),
            FranchiseTab::Others => "Outros".to_owned(),
        }
    }

    /// Nome estável para a interface (`tv`, `movies`, `specials`, `others`).
    pub fn id(self) -> &'static str {
        match self {
            FranchiseTab::Tv => "tv",
            FranchiseTab::Movies => "movies",
            FranchiseTab::Specials => "specials",
            FranchiseTab::Others => "others",
        }
    }
}

/// Abas não vazias, na ordem Séries TV · Filmes · Especiais · Outros, com os
/// índices das obras em ordem de `(ano, ordem)` (empate mantém a ordem de entrada).
/// Cada obra vem com o nome lido e o formato do AniList, se já se sabe.
pub fn franchise_tabs(works: &[(&WorkName, Option<&str>)]) -> Vec<(FranchiseTab, Vec<usize>)> {
    [FranchiseTab::Tv, FranchiseTab::Movies, FranchiseTab::Specials, FranchiseTab::Others]
        .into_iter()
        .filter_map(|tab| {
            let mut ix: Vec<usize> = (0..works.len()).filter(|&i| FranchiseTab::of(works[i].0, works[i].1) == tab).collect();
            ix.sort_by_key(|&i| (works[i].0.year, works[i].0.order));
            (!ix.is_empty()).then_some((tab, ix))
        })
        .collect()
}

/// `Franquia · 2004–2019`, com os anos das obras.
pub fn franchise_span(names: &[&WorkName]) -> String {
    let years = names.iter().filter_map(|w| w.year);
    match (years.clone().min(), years.max()) {
        (Some(a), Some(b)) if a != b => format!("Franquia · {a}–{b}"),
        (Some(a), _) => format!("Franquia · {a}"),
        _ => "Franquia".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::parse_work_name;

    fn tabs(works: &[(&str, Option<&str>)]) -> Vec<(FranchiseTab, Vec<usize>)> {
        let names: Vec<WorkName> = works.iter().map(|(n, _)| parse_work_name(n)).collect();
        let input: Vec<(&WorkName, Option<&str>)> = names.iter().zip(works).map(|(n, (_, f))| (n, *f)).collect();
        franchise_tabs(&input)
    }

    #[test]
    fn franchise_tabs_by_kind() {
        let got = tabs(&[
            ("[2005.1] B [FILME]", None),
            ("[2019.0] Star Twinkle Precure [FILME]", Some("TV")),
            ("Extras sem ano", None),
            ("[2004] A [TV]", None),
        ]);
        assert_eq!(got, vec![(FranchiseTab::Tv, vec![3, 1]), (FranchiseTab::Movies, vec![0]), (FranchiseTab::Others, vec![2])]);
    }

    #[test]
    fn franchise_tabs_other_kinds_and_fallbacks() {
        let got = tabs(&[
            ("[2008] Splash [OVA]", None),
            ("[2009] Gala [ESPECIAL]", None),
            ("[2010] Web [ONA]", None),
            ("[2011] Sem tag", None),
            ("[2012] Filme [FILME]", Some("FORMATO_NOVO")),
            ("[2013] Curta [TV]", Some("OVA")),
        ]);
        assert_eq!(got, vec![(FranchiseTab::Tv, vec![2, 3]), (FranchiseTab::Movies, vec![4]), (FranchiseTab::Specials, vec![0, 1, 5])]);
    }

    #[test]
    fn span_of_years() {
        let a = parse_work_name("[2004] A [TV]");
        let b = parse_work_name("[2019] B [TV]");
        let none = parse_work_name("Extras");
        assert_eq!(franchise_span(&[&a, &b, &none]), "Franquia · 2004–2019");
        assert_eq!(franchise_span(&[&a]), "Franquia · 2004");
        assert_eq!(franchise_span(&[&none]), "Franquia");
    }
}
