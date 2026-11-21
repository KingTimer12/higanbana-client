//! Regras da barra de filtros do catálogo (sem interface): rótulos, anos,
//! gêneros oferecidos, lista filtrada e ordenada, avisos e linha de status.

use std::{collections::HashSet, time::Duration};

use higanbana_core::{
    anilist::types::MediaSummary,
    anime_index::IndexEntry,
    catalog_filter::{judge, sort_key_score, sort_key_year, Filters, Season, Sort, Verdict},
    library::{is_dated, split_path, WorkKind},
    nuvem::SearchResult,
};

use crate::series::Series;

/// Limites aceitos nos campos de ano.
pub const YEAR_MIN: i32 = 1900;
pub const YEAR_MAX: i32 = 2100;

pub const KINDS: [WorkKind; 5] = [WorkKind::Tv, WorkKind::Movie, WorkKind::Ova, WorkKind::Ona, WorkKind::Special];
pub const SEASONS: [Season; 4] = [Season::Winter, Season::Spring, Season::Summer, Season::Fall];
pub const SORTS: [Sort; 3] = [Sort::Az, Sort::Year, Sort::Score];

pub fn kind_label(k: WorkKind) -> &'static str {
    match k {
        WorkKind::Tv => "TV",
        WorkKind::Movie => "Filme",
        WorkKind::Ova => "OVA",
        WorkKind::Ona => "ONA",
        WorkKind::Special => "Especial",
    }
}

pub fn season_label(s: Season) -> &'static str {
    match s {
        Season::Winter => "Inverno",
        Season::Spring => "Primavera",
        Season::Summer => "Verão",
        Season::Fall => "Outono",
    }
}

pub fn sort_label(s: Sort) -> &'static str {
    match s {
        Sort::Az => "A–Z",
        Sort::Year => "Ano",
        Sort::Score => "Nota",
    }
}

/// Ano de quatro dígitos dentro dos limites; o resto não conta.
pub fn parse_year(text: &str) -> Option<i32> {
    let t = text.trim();
    if t.len() != 4 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok().filter(|y| (YEAR_MIN..=YEAR_MAX).contains(y))
}

/// Faixa de anos dos campos "de"/"até": invertida vira na ordem certa; um lado vazio fica aberto.
pub fn year_range(from: Option<i32>, to: Option<i32>) -> Option<(i32, i32)> {
    match (from, to) {
        (None, None) => None,
        (from, to) => {
            let (a, b) = (from.unwrap_or(YEAR_MIN), to.unwrap_or(YEAR_MAX));
            Some((a.min(b), a.max(b)))
        }
    }
}

/// Texto do chip de ano.
pub fn year_label((from, to): (i32, i32)) -> String {
    match (from, to) {
        _ if from == to => from.to_string(),
        (YEAR_MIN, to) => format!("Até {to}"),
        (from, YEAR_MAX) => format!("Desde {from}"),
        (from, to) => format!("{from}–{to}"),
    }
}

/// Gêneros dos metadados já carregados, sem repetir (sem diferenciar caixa), em ordem alfabética.
pub fn loaded_genres<'a>(metas: impl IntoIterator<Item = &'a MediaSummary>) -> Vec<String> {
    // Mesmo resultado de `genres_to_offer(&[], todos)`, copiando só os gêneros novos.
    let mut seen = HashSet::new();
    let mut genres: Vec<String> = metas.into_iter().flat_map(|m| &m.genres).filter(|g| seen.insert(g.to_lowercase())).cloned().collect();
    genres.sort_by_cached_key(|g| g.to_lowercase());
    genres
}

/// Os gêneros carregados mais os escolhidos (que sempre entram), em ordem alfabética.
pub fn genres_to_offer(loaded: &[String], selected: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut genres: Vec<String> = loaded.iter().chain(selected).filter(|g| seen.insert(g.to_lowercase())).cloned().collect();
    genres.sort_by_cached_key(|g| g.to_lowercase());
    genres
}

/// Liga ou desliga `x` na lista.
pub fn toggle<T: PartialEq>(list: &mut Vec<T>, x: T) {
    match list.iter().position(|y| *y == x) {
        Some(pos) => {
            list.remove(pos);
        }
        None => list.push(x),
    }
}

/// "Gênero · 2" quando há escolhas.
pub fn trigger_label(name: &str, count: usize) -> String {
    if count == 0 { name.to_owned() } else { format!("{name} · {count}") }
}

/// Número com separador de milhar: 4031 → "4.031".
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// "N obras · metadados X / Y · índice X / 27": metadados e índice só enquanto incompletos.
pub fn status_line(works: usize, (resolved, total): (usize, usize), (indexed, letters): (usize, usize)) -> String {
    use std::fmt::Write as _;
    let mut line = format!("{} {}", thousands(works), if works == 1 { "obra" } else { "obras" });
    // `write!` no mesmo texto, sem um `String` temporário por trecho.
    if resolved < total {
        let _ = write!(line, " · metadados {} / {}", thousands(resolved), thousands(total));
    }
    if indexed < letters {
        let _ = write!(line, " · índice {indexed} / {letters}");
    }
    line
}

/// Aviso no contador de metadados enquanto o AniList está em pausa (segundos arredondados para cima).
pub fn anilist_pause_line(left: Duration) -> String {
    format!("AniList pediu uma pausa — continua em {} s", left.as_millis().div_ceil(1000).max(1))
}

/// Aviso de lista vazia.
#[derive(Debug, PartialEq, Eq)]
pub enum EmptyNotice {
    /// "Nenhuma obra com “…”" (busca por letra).
    NoMatch,
    /// "Nenhuma obra com esses filtros" (grade única).
    NoFilterMatch,
}

/// Qual aviso de lista vazia mostrar; nenhum quando a grade tem obras ou quando
/// a busca na nuvem achou obras dentro de franquias (o aviso as contradiria).
pub fn empty_notice(flat: bool, searching: bool, nothing_shown: bool, franchise_found: bool) -> Option<EmptyNotice> {
    match (nothing_shown && !franchise_found, flat, searching) {
        (false, _, _) => None,
        (true, true, _) => Some(EmptyNotice::NoFilterMatch),
        (true, false, true) => Some(EmptyNotice::NoMatch),
        (true, false, false) => None,
    }
}

/// Texto dos campos "de"/"até" para uma faixa guardada (lado aberto fica vazio).
pub fn year_fields_text(years: Option<(i32, i32)>) -> (String, String) {
    let Some((from, to)) = years else { return (String::new(), String::new()) };
    let side = |y: i32, open: i32| if y == open { String::new() } else { y.to_string() };
    (side(from, YEAR_MIN), side(to, YEAR_MAX))
}

/// Uma obra do índice com o que se sabe dela.
pub struct Work<'a> {
    pub entry: &'a IndexEntry,
    pub meta: Option<&'a MediaSummary>,
    /// Obras de dentro da franquia, quando ela já foi aberta.
    pub inner: Vec<MediaSummary>,
}

/// Obras que passam nos filtros (as sem resposta ainda ficam de fora), na ordem pedida.
/// A–Z mantém a ordem do índice; Ano e Nota vão do maior ao menor, e o empate fica em A–Z.
pub fn filtered_works(works: &[Work], f: &Filters, sort: Sort) -> Vec<IndexEntry> {
    let mut found: Vec<&Work> = works.iter().filter(|w| judge(&w.entry.name, w.meta, &w.inner, f) == Verdict::Pass).collect();
    // A franquia vale pela melhor das obras conhecidas (a própria pasta ou as de dentro).
    let best = |w: &Work, key: fn(Option<&MediaSummary>) -> i64| w.meta.into_iter().chain(&w.inner).map(|m| key(Some(m))).max().unwrap_or_else(|| key(None));
    match sort {
        Sort::Az => {}
        Sort::Year => found.sort_by_key(|w| std::cmp::Reverse(best(w, |m| sort_key_year(m) as i64))),
        Sort::Score => found.sort_by_key(|w| std::cmp::Reverse(best(w, sort_key_score))),
    }
    found.into_iter().map(|w| w.entry.clone()).collect()
}

/// Resultados da busca na nuvem que são obras dentro de franquias da seção (`<raiz>/Letra/Franquia/[Ano] Obra`).
/// Pastas mais fundas (versões, extras, episódios) e subpastas sem ano ficam de fora.
pub fn franchise_results(results: Vec<SearchResult>, series: Series) -> Vec<SearchResult> {
    let depth = split_path(series.root).len() + 3;
    results
        .into_iter()
        .filter(|r| {
            let parts = split_path(&r.path);
            r.kind == series.search_kind && parts.len() == depth && is_dated(parts[depth - 1])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use higanbana_core::nuvem::SearchKind;
    use higanbana_core::anime_index::IndexEntry;
    use higanbana_core::catalog_filter::{Filters, Sort};
    use serde_json::json;

    fn summary(v: serde_json::Value) -> MediaSummary {
        serde_json::from_value(v).unwrap()
    }

    fn entry(name: &str) -> IndexEntry {
        IndexEntry { letter: "Letra X".into(), name: name.into(), path: format!("Animes/Letra X/{name}") }
    }

    #[test]
    fn labels() {
        assert_eq!(KINDS.map(kind_label), ["TV", "Filme", "OVA", "ONA", "Especial"]);
        assert_eq!(SEASONS.map(season_label), ["Inverno", "Primavera", "Verão", "Outono"]);
        assert_eq!(SORTS.map(sort_label), ["A–Z", "Ano", "Nota"]);
        assert_eq!(trigger_label("Gênero", 0), "Gênero");
        assert_eq!(trigger_label("Gênero", 1), "Gênero · 1");
    }

    #[test]
    fn years() {
        assert_eq!(parse_year(" 2010 "), Some(2010));
        assert_eq!(parse_year("201"), None);
        assert_eq!(parse_year("abcd"), None);
        assert_eq!(parse_year("1500"), None);
        assert_eq!(year_range(None, None), None);
        assert_eq!(year_range(Some(2009), Some(2011)), Some((2009, 2011)));
        // Invertida: troca.
        assert_eq!(year_range(Some(2011), Some(2009)), Some((2009, 2011)));
        assert_eq!(year_range(Some(2009), None), Some((2009, YEAR_MAX)));
        assert_eq!(year_range(None, Some(2011)), Some((YEAR_MIN, 2011)));
        assert_eq!(year_label((2009, 2011)), "2009–2011");
        assert_eq!(year_label((2010, 2010)), "2010");
        assert_eq!(year_label((2009, YEAR_MAX)), "Desde 2009");
        assert_eq!(year_label((YEAR_MIN, 2011)), "Até 2011");
    }

    #[test]
    fn offers_loaded_genres() {
        let a = summary(json!({"id":1,"genres":["Mahou Shoujo","Comedy"]}));
        let b = summary(json!({"id":2,"genres":["action","Comedy"]}));
        let loaded = loaded_genres([&a, &b]);
        assert_eq!(loaded, vec!["action", "Comedy", "Mahou Shoujo"]);
        assert_eq!(genres_to_offer(&loaded, &[]), loaded);
        // O escolhido continua no menu mesmo sem obra carregada com ele; repetido em outra caixa não duplica.
        assert_eq!(genres_to_offer(&loaded, &["Sports".to_owned(), "comedy".to_owned()]), vec!["action", "Comedy", "Mahou Shoujo", "Sports"]);
        assert!(genres_to_offer(&[], &[]).is_empty());
    }

    #[test]
    fn toggles() {
        let mut v = vec![1, 2];
        toggle(&mut v, 3);
        toggle(&mut v, 1);
        assert_eq!(v, vec![2, 3]);
    }

    #[test]
    fn status() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(4031), "4.031");
        assert_eq!(thousands(1234567), "1.234.567");
        assert_eq!(status_line(74, (3120, 4031), (27, 27)), "74 obras · metadados 3.120 / 4.031");
        assert_eq!(status_line(1, (10, 10), (27, 27)), "1 obra");
        assert_eq!(status_line(0, (0, 0), (27, 27)), "0 obras");
        // Enquanto indexa, o índice aparece para o filtro não esconder obras em silêncio.
        assert_eq!(status_line(5, (10, 10), (3, 27)), "5 obras · índice 3 / 27");
        assert_eq!(status_line(5, (8, 10), (3, 27)), "5 obras · metadados 8 / 10 · índice 3 / 27");
    }

    #[test]
    fn pause_notice() {
        assert_eq!(anilist_pause_line(Duration::from_secs(11)), "AniList pediu uma pausa — continua em 11 s");
        assert_eq!(anilist_pause_line(Duration::from_millis(10_200)), "AniList pediu uma pausa — continua em 11 s");
        // Quase acabando: nunca "0 s".
        assert_eq!(anilist_pause_line(Duration::from_millis(5)), "AniList pediu uma pausa — continua em 1 s");
    }

    #[test]
    fn filters_and_sorts() {
        let (a, b, c, d) = (entry("Aria"), entry("Bleach [DUAL]"), entry("Clannad"), entry("Dragon Ball"));
        let tv2010 = summary(json!({"id":1,"format":"TV","seasonYear":2010,"averageScore":70,"genres":["Comedy"]}));
        let movie2015 = summary(json!({"id":2,"format":"MOVIE","seasonYear":2015,"averageScore":85,"genres":["Action"]}));
        let tv2005 = summary(json!({"id":3,"format":"TV","seasonYear":2005,"genres":["Drama"]}));
        let works = vec![
            Work { entry: &a, meta: Some(&tv2010), inner: vec![] },
            Work { entry: &b, meta: Some(&movie2015), inner: vec![] },
            // Sem metadados ainda.
            Work { entry: &c, meta: None, inner: vec![] },
            // Franquia sem metadados próprios, com uma obra de dentro conhecida.
            Work { entry: &d, meta: None, inner: vec![tv2005] },
        ];
        let names = |v: Vec<IndexEntry>| v.into_iter().map(|e| e.name).collect::<Vec<_>>();

        // Sem filtro: todas, na ordem do índice.
        assert_eq!(names(filtered_works(&works, &Filters::default(), Sort::Az)), vec!["Aria", "Bleach [DUAL]", "Clannad", "Dragon Ball"]);
        // TV: a sem metadados some; a franquia entra pela obra de dentro.
        let tv = Filters { kinds: vec![WorkKind::Tv], ..Default::default() };
        assert_eq!(names(filtered_works(&works, &tv, Sort::Az)), vec!["Aria", "Dragon Ball"]);
        // Dublado não depende de metadados.
        let dub = Filters { dubbed: true, ..Default::default() };
        assert_eq!(names(filtered_works(&works, &dub, Sort::Az)), vec!["Bleach [DUAL]"]);
        // Ano: mais novo primeiro (a franquia vale pela obra de dentro); sem ano no fim.
        assert_eq!(names(filtered_works(&works, &Filters::default(), Sort::Year)), vec!["Bleach [DUAL]", "Aria", "Dragon Ball", "Clannad"]);
        // Nota: maior primeiro; empate (sem nota) fica em A–Z.
        assert_eq!(names(filtered_works(&works, &Filters::default(), Sort::Score)), vec!["Bleach [DUAL]", "Aria", "Clannad", "Dragon Ball"]);
        // Busca por nome junto com os filtros.
        let q = Filters { query: "clan".into(), ..Default::default() };
        assert_eq!(names(filtered_works(&works, &q, Sort::Year)), vec!["Clannad"]);
    }

    #[test]
    fn empty_state_never_contradicts_franchise_results() {
        // Algo na grade: nada de estado vazio.
        assert_eq!(empty_notice(true, true, false, false), None);
        // Busca sem filtro que não achou nada por letra.
        assert_eq!(empty_notice(false, true, true, false), Some(EmptyNotice::NoMatch));
        // Sem busca nem filtro, a lista vazia é só o índice carregando.
        assert_eq!(empty_notice(false, false, true, false), None);
        // Grade única vazia.
        assert_eq!(empty_notice(true, false, true, false), Some(EmptyNotice::NoFilterMatch));
        // Achou dentro de franquias: o aviso some nos dois modos.
        assert_eq!(empty_notice(true, true, true, true), None);
        assert_eq!(empty_notice(false, true, true, true), None);
    }

    #[test]
    fn restores_year_fields() {
        assert_eq!(year_fields_text(None), (String::new(), String::new()));
        assert_eq!(year_fields_text(Some((2009, 2011))), ("2009".into(), "2011".into()));
        assert_eq!(year_fields_text(Some((2009, YEAR_MAX))), ("2009".into(), String::new()));
        assert_eq!(year_fields_text(Some((YEAR_MIN, 2011))), (String::new(), "2011".into()));
    }

    #[test]
    fn keeps_works_inside_franchises() {
        let r = |path: &str, kind| SearchResult { name: path.rsplit('/').next().unwrap().into(), path: path.into(), kind };
        let found = franchise_results(
            vec![
            r("Animes/Letra P/Precure", SearchKind::Anime),
            r("Animes/Letra P/Precure/[2010] Heartcatch Precure!", SearchKind::Anime),
            r("Mangas/Letra P/Precure/Vol 1", SearchKind::Manga),
            r("/Animes/Letra H/Hunter x Hunter/[2011] Hunter x Hunter/", SearchKind::Anime),
            // Versão, extras e episódios dentro da obra não são obras.
            r("Animes/Letra P/Precure/[2010] Heartcatch Precure!/[BD] 1080p", SearchKind::Anime),
            r("Animes/Letra P/Precure/[2010] Heartcatch Precure!/Extras", SearchKind::Anime),
            // Subpasta da franquia sem ano.
            r("Animes/Letra P/Precure/Extras", SearchKind::Anime),
            ],
            crate::series::ANIME,
        );
        let paths: Vec<&str> = found.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, vec!["Animes/Letra P/Precure/[2010] Heartcatch Precure!", "/Animes/Letra H/Hunter x Hunter/[2011] Hunter x Hunter/"]);
    }
}

#[test]
fn keeps_manga_works_inside_franchises() {
    use higanbana_core::nuvem::SearchKind;
    use higanbana_core::anime_index::IndexEntry;
    use higanbana_core::catalog_filter::{Filters, Sort};
    let r = |path: &str, kind| SearchResult { name: path.rsplit('/').next().unwrap().into(), path: path.into(), kind };
    let found = franchise_results(
        vec![
            r("Mangás/Séries Mangás/Letra G/Gantz/[2000] Gantz", SearchKind::Manga),
            r("Mangás/Séries Mangás/Letra G/Gantz", SearchKind::Manga),
            r("Animes/Letra G/Gantz/[2004] Gantz", SearchKind::Anime),
        ],
        crate::series::MANGA,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "Mangás/Séries Mangás/Letra G/Gantz/[2000] Gantz");
}
