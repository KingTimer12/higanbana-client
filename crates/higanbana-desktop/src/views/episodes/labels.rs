//! Textos dos cards de episódio: rótulo e título, segunda linha quando o rótulo
//! se repete, data do TMDB, "EPISÓDIO 1 · DATA" e o número grande.

use higanbana_core::anilist::{text, types::FuzzyDate};
use higanbana_core::library::parse_episode;
use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::logic::episode_number;
use higanbana_core::tmdb::EpisodeView;

/// Rótulo ("E1") e título do card. Sem episódio do TMDB: "Episódio N" quando o
/// arquivo tem número; senão o nome do arquivo, sem rótulo. O número do rótulo
/// é o do arquivo (o que a pessoa vê no player); o do TMDB só quando o arquivo
/// não tem número legível.
/// Obra de um episódio só (`single`, filme ou especial único) ligada ao TMDB:
/// sem rótulo, porque o número no nome ("Filme 02", "OVA 03") não é de episódio.
pub fn card_text(file: &FileEntry, ep: Option<&EpisodeView>, single: bool) -> (String, String) {
    if let Some(e) = ep.filter(|_| single) {
        return (String::new(), e.title.clone());
    }
    let number = episode_number(&file.name).or_else(|| ep.map(|e| e.number));
    let label = number.map(|n| format!("E{n}")).unwrap_or_default();
    let title = match (ep, number) {
        (Some(e), _) => e.title.clone(),
        (None, Some(n)) => format!("Episódio {n}"),
        (None, None) => file.name.clone(),
    };
    (label, title)
}

/// Segunda linha dos vídeos cujo rótulo se repete (`01` e `01v2`, 720p e 1080p),
/// para `(rótulo, nome do arquivo)`: a qualidade do nome quando ela separa o
/// grupo; senão o nome do arquivo. Rótulo único ou vazio → `None`.
pub fn disambiguators(items: &[(String, String)]) -> Vec<Option<String>> {
    let quality: Vec<Option<String>> = items.iter().map(|(_, name)| parse_episode(name).quality).collect();
    items
        .iter()
        .enumerate()
        .map(|(i, (label, name))| {
            if label.is_empty() {
                return None;
            }
            let group: Vec<usize> = (0..items.len()).filter(|&j| items[j].0 == *label).collect();
            if group.len() < 2 {
                return None;
            }
            let distinct = group.iter().all(|&j| quality[j].is_some())
                && group.iter().all(|&j| group.iter().filter(|&&k| quality[k] == quality[j]).count() == 1);
            Some(if distinct { quality[i].clone().unwrap_or_default() } else { name.clone() })
        })
        .collect()
}

/// Data de exibição do TMDB (`2023-09-29`) no formato das datas do AniList ("29 set 2023").
pub fn air_date(date: Option<&str>) -> Option<String> {
    let mut parts = date?.split('-').map(|p| p.parse::<u32>().ok());
    let year = parts.next().flatten()? as i32;
    let month = parts.next().flatten();
    let day = parts.next().flatten();
    text::date(Some(FuzzyDate { year: Some(year), month, day }))
}

/// Colunas da lista: metade das da grade (um card da lista vale dois da grade).
pub fn list_columns(grid_cols: usize) -> usize {
    (grid_cols / 2).max(1)
}

/// Linha de cima do card da lista: "EPISÓDIO 1 · 29 SET 2023".
pub fn kicker(label: &str, date: Option<&str>) -> String {
    let ep = label.strip_prefix('E').filter(|n| !n.is_empty()).map(|n| format!("Episódio {n}"));
    [ep, date.map(str::to_owned)].into_iter().flatten().collect::<Vec<_>>().join(" · ").to_uppercase()
}

/// Número grande do card da grade: "E1" vira "01". Sem número, nada.
pub fn head_number(label: &str) -> Option<String> {
    label.strip_prefix('E')?.parse::<u32>().ok().map(|n| format!("{n:02}"))
}

