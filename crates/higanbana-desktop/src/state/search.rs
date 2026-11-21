//! Busca do acervo (paleta Ctrl K). Os resultados vêm da nuvem: cada consulta
//! pede uma busca nova, 250 ms depois da última tecla; resposta de consulta
//! antiga é descartada.

use std::time::Duration;

use higanbana_core::library::media_for_kind;
use higanbana_core::nuvem::{SearchKind, SearchResult};
use higanbana_gpui::components::CommandState;

use super::AppState;

const DEBOUNCE: Duration = Duration::from_millis(250);
/// Quantos resultados pedem capa ao AniList.
const THUMBS: usize = 8;

pub const KIND_ORDER: [SearchKind; 7] =
    [SearchKind::Anime, SearchKind::Manga, SearchKind::Dorama, SearchKind::Desenho, SearchKind::Ost, SearchKind::Folder, SearchKind::Other];

pub fn kind_heading(k: SearchKind) -> &'static str {
    match k {
        SearchKind::Anime => "Animes",
        SearchKind::Manga => "Mangás",
        SearchKind::Dorama => "Doramas",
        SearchKind::Ost => "OSTs",
        SearchKind::Desenho => "Desenhos",
        SearchKind::Folder => "Pastas",
        SearchKind::Other => "Outros",
    }
}

/// Resultados agrupados por tipo, na ordem da paleta (grupos vazios somem).
pub fn group_results(results: Vec<SearchResult>) -> Vec<(SearchKind, Vec<SearchResult>)> {
    KIND_ORDER
        .iter()
        .filter_map(|&kind| {
            let items: Vec<SearchResult> = results.iter().filter(|r| r.kind == kind).cloned().collect();
            (!items.is_empty()).then_some((kind, items))
        })
        .collect()
}

#[derive(Default)]
pub struct Search {
    pub open: bool,
    pub cmd: CommandState,
    /// Consulta atual (aparada).
    pub query: String,
    pub groups: Vec<(SearchKind, Vec<SearchResult>)>,
    pending: Option<tokio::task::JoinHandle<()>>,
}

impl Search {
    /// Resposta de `asked`: só vale se ainda é a consulta atual.
    pub fn accept(&mut self, asked: &str, results: Vec<SearchResult>) -> bool {
        if self.query != asked {
            return false;
        }
        self.groups = group_results(results);
        self.cmd.set_loading(false);
        self.cmd.reset_selection();
        true
    }

    /// Fecha e esquece a consulta: a próxima abertura vem limpa e uma resposta
    /// atrasada não acha mais a consulta (é descartada).
    pub fn close(&mut self) {
        self.open = false;
        if let Some(h) = self.pending.take() {
            h.abort();
        }
        self.cmd.clear();
        self.query.clear();
        self.groups.clear();
    }

    /// Abre; `false` se já estava aberta (Ctrl K de novo não reabre).
    pub fn open(&mut self) -> bool {
        if self.open {
            return false;
        }
        self.open = true;
        true
    }

    /// Resultado na posição `(grupo, item)`.
    pub fn at(&self, (g, i): (usize, usize)) -> Option<&SearchResult> {
        self.groups.get(g).and_then(|(_, items)| items.get(i))
    }
}

impl AppState {
    /// Abre a paleta limpa (sem efeito se já está aberta).
    pub fn open_search(&mut self) -> bool {
        self.search.open()
    }

    pub fn close_search(&mut self) {
        self.search.close();
    }

    /// Texto da busca mudou.
    pub fn on_search_query(&mut self, q: &str) {
        let q = q.trim().to_owned();
        if let Some(h) = self.search.pending.take() {
            h.abort();
        }
        self.search.query.clone_from(&q);
        if q.is_empty() {
            self.search.groups.clear();
            self.search.cmd.set_loading(false);
            return;
        }
        self.search.cmd.set_loading(true);
        self.search.pending = Some(self.after(DEBOUNCE, move |s, _| s.run_search(q)));
    }

    fn run_search(&mut self, q: String) {
        let core = self.core.clone();
        let asked = q.clone();
        let handle = self.run(async move { core.cloud().search(&q).await.map(|r| r.results).unwrap_or_default() }, move |s, results, _| {
            if s.search.accept(&asked, results) {
                s.want_result_covers();
            }
        });
        self.search.pending = Some(handle);
    }

    /// Capas do AniList para os primeiros resultados.
    fn want_result_covers(&mut self) {
        let wanted: Vec<_> = self
            .search
            .groups
            .iter()
            .flat_map(|(_, items)| items.iter())
            .filter_map(|r| media_for_kind(r.kind).map(|m| (r.name.clone(), m)))
            .take(THUMBS)
            .collect();
        for (name, media) in wanted {
            self.want_lookup(&name, media);
        }
    }

    /// Enter/clique num resultado: fecha e navega.
    pub fn confirm_search(&mut self, ix: (usize, usize)) {
        let Some(path) = self.search.at(ix).map(|r| r.path.clone()) else { return };
        self.close_search();
        self.go(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str, kind: SearchKind) -> SearchResult {
        SearchResult { name: name.into(), path: format!("x/{name}"), kind }
    }

    #[test]
    fn agrupa_na_ordem_da_paleta() {
        let g = group_results(vec![r("b", SearchKind::Manga), r("a", SearchKind::Anime), r("c", SearchKind::Manga)]);
        let kinds: Vec<_> = g.iter().map(|(k, items)| (*k, items.len())).collect();
        assert_eq!(kinds, vec![(SearchKind::Anime, 1), (SearchKind::Manga, 2)]);
        assert_eq!(kind_heading(SearchKind::Ost), "OSTs");
    }

    #[test]
    fn ctrl_k_com_paleta_aberta_nao_reabre() {
        let mut s = Search::default();
        assert!(s.open());
        assert!(!s.open(), "segunda vez não reabre nem refoca");
        s.open = false;
        assert!(s.open());
    }

    #[test]
    fn busca_descarta_resposta_antiga() {
        let mut s = Search { query: "naruto".into(), ..Default::default() };
        s.cmd.set_loading(true);
        assert!(!s.accept("nar", vec![r("Velho", SearchKind::Anime)]), "consulta antiga");
        assert!(s.groups.is_empty());
        assert!(s.cmd.loading, "continua buscando a atual");
        assert!(s.accept("naruto", vec![r("Naruto", SearchKind::Anime)]));
        assert_eq!(s.at((0, 0)).map(|x| x.name.as_str()), Some("Naruto"));
        assert!(!s.cmd.loading);
    }
}

#[cfg(test)]
mod close_tests {
    use super::*;

    #[test]
    fn fechar_limpa_para_reabrir() {
        let mut s = Search::default();
        s.open();
        s.cmd.input.set_text("naruto");
        s.query = "naruto".into();
        s.cmd.set_loading(true);
        s.groups = group_results(vec![SearchResult { name: "Naruto".into(), path: "a".into(), kind: SearchKind::Anime }]);
        s.close();
        assert!(!s.open && s.query.is_empty() && s.groups.is_empty());
        assert_eq!(s.cmd.query(), "");
        assert!(!s.cmd.loading, "sem spinner preso");
        assert!(!s.accept("naruto", vec![]), "resposta atrasada da consulta fechada é descartada");
    }
}
