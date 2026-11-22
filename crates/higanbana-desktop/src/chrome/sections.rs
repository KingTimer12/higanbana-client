//! Seções da raiz do acervo: rótulos, ícones, ordem e destinos da navegação.

use higanbana_core::library::{natural_cmp, split_path};

/// Pasta das revistas, dentro de `Mangás` (fica separada das séries na navegação).
pub const MAGAZINES_PATH: &str = "Mangás/Revistas Antigas sobre Animes";
/// Catálogo de séries de mangá.
pub const MANGA_ROOT: &str = higanbana_core::anime_index::MANGA_ROOT;

/// Rótulo de seção na barra lateral, no início e nas migalhas.
pub fn section_label(name: &str) -> String {
    match name {
        "[#Temp]" => "Recentes".into(),
        "AniGUI (Downloader Oficial)" => "AniGUI".into(),
        n => n.into(),
    }
}

/// Ícone e descrição de cada seção da raiz.
pub fn section_meta(name: &str) -> (&'static str, &'static str) {
    match name {
        "Animes" => ("clapperboard", "Séries, filmes e OVAs, por letra."),
        "Mangás" => ("book-open", "Séries completas e volumes avulsos."),
        "Doramas" => ("tv", "Dramas asiáticos com legenda."),
        "Desenhos" => ("palette", "Animações ocidentais."),
        "OSTs" => ("music", "Trilhas de animes, jogos e filmes."),
        "[#Temp]" => ("inbox", "Envios novos, antes de organizar."),
        _ => ("folder-open", ""),
    }
}

/// Um destino da navegação (barra lateral e atalhos do início).
#[derive(Clone, Debug, PartialEq)]
pub struct NavEntry {
    pub label: String,
    pub icon: &'static str,
    /// Rota aberta ao clicar.
    pub target: String,
    /// Seção da raiz a que pertence.
    pub section: String,
}

/// Seções da raiz viram destinos; `Mangás` se divide em séries (catálogo) e revistas.
pub fn nav_entries(sections: &[String]) -> Vec<NavEntry> {
    let entry = |label: &str, icon, target: &str, section: &str| NavEntry { label: label.to_owned(), icon, target: target.to_owned(), section: section.to_owned() };
    sections
        .iter()
        .flat_map(|name| match name.as_str() {
            "Mangás" => vec![entry("Mangás", "book-open", MANGA_ROOT, name), entry("Revistas", "newspaper", MAGAZINES_PATH, name)],
            _ => vec![entry(&section_label(name), section_meta(name).0, name, name)],
        })
        .collect()
}

/// Destino ativo para a rota: o de prefixo mais longo (`Mangás/Revistas…` vence `Mangás`).
pub fn active_nav(entries: &[NavEntry], path: &str) -> Option<usize> {
    let segs = split_path(path);
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            let t = split_path(&e.target);
            segs.len() >= t.len() && segs[..t.len()] == t[..]
        })
        .max_by_key(|(_, e)| split_path(&e.target).len())
        .map(|(i, _)| i)
        // Pasta `Mangás` aberta direto: fica no destino da seção.
        .or_else(|| segs.first().and_then(|first| entries.iter().position(|e| e.section == *first)))
}

/// Ordem na barra lateral: conteúdo principal primeiro.
pub fn section_order(a: &str, b: &str) -> std::cmp::Ordering {
    const ORDER: [&str; 7] = ["Animes", "Mangás", "Doramas", "Desenhos", "OSTs", "[#Temp]", "AniGUI (Downloader Oficial)"];
    let pos = |s: &str| ORDER.iter().position(|o| *o == s).unwrap_or(99);
    pos(a).cmp(&pos(b)).then_with(|| natural_cmp(a, b))
}

/// Seções da raiz em ordem de leitura.
pub fn sort_sections(mut names: Vec<String>) -> Vec<String> {
    names.sort_by(|a, b| section_order(a, b));
    names
}

/// Seções com conteúdo do acervo (o resto é organização da nuvem).
pub fn is_library_section(name: &str) -> bool {
    matches!(name, "Animes" | "Mangás" | "Doramas" | "Desenhos" | "OSTs")
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024. && i < UNITS.len() - 1 {
        v /= 1024.;
        i += 1;
    }
    let num = if i >= 3 { format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_owned() } else { format!("{v:.0}") };
    format!("{} {}", num.replace('.', ","), UNITS[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mangas_split_into_series_and_magazines() {
        let sections: Vec<String> = ["Animes", "Mangás", "Doramas"].map(String::from).into();
        let entries = nav_entries(&sections);
        let targets: Vec<&str> = entries.iter().map(|e| e.target.as_str()).collect();
        assert_eq!(targets, ["Animes", "Mangás/Séries Mangás", MAGAZINES_PATH, "Doramas"]);
        assert_eq!(entries[2].label, "Revistas");
        assert_eq!(active_nav(&entries, "Mangás/Séries Mangás/Letra G/Gantz"), Some(1));
        assert_eq!(active_nav(&entries, "Mangás/Revistas Antigas sobre Animes/Animage"), Some(2));
        assert_eq!(active_nav(&entries, "Mangás"), Some(1));
        assert_eq!(active_nav(&entries, "Animes/Letra B"), Some(0));
    }

    #[test]
    fn secoes_em_ordem_de_leitura() {
        let s = sort_sections(["[#Temp]", "OSTs", "Animes", "Zeta", "Mangás"].map(String::from).into());
        assert_eq!(s, ["Animes", "Mangás", "OSTs", "[#Temp]", "Zeta"]);
    }

    #[test]
    fn acervo_e_nuvem() {
        assert!(is_library_section("Animes") && is_library_section("OSTs"));
        assert!(!is_library_section("[#Temp]"));
    }

    #[test]
    fn bytes_legiveis() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "2 KB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024 / 2), "1,5 GB");
    }
}
