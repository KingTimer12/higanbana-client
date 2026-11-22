//! Fila de downloads: no máximo `MAX_DOWNLOADS` ao mesmo tempo, o resto espera.
//! Só a lista (pura); quem dispara as tarefas no núcleo é `AppState`.

use std::path::PathBuf;

pub const MAX_DOWNLOADS: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlStatus {
    Queued,
    Running,
    Done,
    Error,
}

#[derive(Clone, Debug)]
pub struct Download {
    pub id: String,
    pub path: String,
    pub name: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub status: DlStatus,
    pub dest: Option<PathBuf>,
    pub error: Option<String>,
    /// Para cancelar a tarefa no núcleo enquanto roda.
    pub(super) abort: Option<tokio::task::AbortHandle>,
}

impl Download {
    pub fn progress(&self) -> f32 {
        self.total.filter(|t| *t > 0).map_or(0., |t| (self.downloaded as f64 / t as f64).min(1.) as f32)
    }

    pub fn active(&self) -> bool {
        matches!(self.status, DlStatus::Running | DlStatus::Queued)
    }
}

#[derive(Default)]
pub struct Downloads {
    pub items: Vec<Download>,
    next: u64,
}

impl Downloads {
    pub fn get(&self, path: &str) -> Option<&Download> {
        self.items.iter().find(|d| d.path == path)
    }

    pub(super) fn by_id(&mut self, id: &str) -> Option<&mut Download> {
        self.items.iter_mut().find(|d| d.id == id)
    }

    pub fn pending(&self) -> usize {
        self.items.iter().filter(|d| d.active()).count()
    }

    /// Progresso agregado dos que estão baixando (0–1).
    pub fn overall(&self) -> f32 {
        let (done, total) = self
            .items
            .iter()
            .filter(|d| d.status == DlStatus::Running)
            .filter_map(|d| Some((d.downloaded, d.total?)))
            .fold((0u64, 0u64), |(a, b), (d, t)| (a + d, b + t));
        if total == 0 { 0. } else { done as f32 / total as f32 }
    }

    /// Põe na fila (ou reenfileira um que terminou/falhou). Mais novo no topo.
    pub fn enqueue(&mut self, path: &str) {
        if let Some(d) = self.items.iter_mut().find(|d| d.path == path) {
            if d.active() {
                return;
            }
            d.status = DlStatus::Queued;
            d.error = None;
            d.downloaded = 0;
            return;
        }
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        self.next += 1;
        let id = format!("dl-{}-{}", std::process::id(), self.next);
        self.items.insert(0, Download { id, path: path.to_owned(), name, downloaded: 0, total: None, status: DlStatus::Queued, dest: None, error: None, abort: None });
    }

    /// Os próximos da fila que cabem agora (mais antigos primeiro), já marcados como baixando.
    pub fn start_next(&mut self) -> Vec<(String, String)> {
        let running = self.items.iter().filter(|d| d.status == DlStatus::Running).count();
        let picked: Vec<(String, String)> = self
            .items
            .iter()
            .rev()
            .filter(|d| d.status == DlStatus::Queued)
            .map(|d| (d.id.clone(), d.path.clone()))
            .take(MAX_DOWNLOADS.saturating_sub(running))
            .collect();
        for (id, _) in &picked {
            if let Some(d) = self.by_id(id) {
                d.status = DlStatus::Running;
                d.error = None;
                d.downloaded = 0;
            }
        }
        picked
    }

    pub fn progress(&mut self, id: &str, downloaded: u64, total: Option<u64>) -> bool {
        match self.by_id(id) {
            Some(d) => {
                d.downloaded = downloaded;
                d.total = total;
                true
            }
            None => false,
        }
    }

    /// Tira da lista um que está na fila ou baixando (devolve o abort da tarefa).
    pub fn cancel(&mut self, id: &str) -> Option<Option<tokio::task::AbortHandle>> {
        let i = self.items.iter().position(|d| d.id == id && d.active())?;
        Some(self.items.remove(i).abort)
    }

    pub fn clear_finished(&mut self) {
        self.items.retain(Download::active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_dois_baixam_juntos() {
        let mut d = Downloads::default();
        for p in ["a/1.mkv", "a/2.mkv", "a/3.mkv"] {
            d.enqueue(p);
        }
        let started = d.start_next();
        assert_eq!(started.iter().map(|s| s.1.as_str()).collect::<Vec<_>>(), vec!["a/1.mkv", "a/2.mkv"], "mais antigos primeiro");
        assert!(d.start_next().is_empty(), "limite de dois");
        assert_eq!(d.pending(), 3);
    }

    #[test]
    fn progresso_de_download_atualiza_item() {
        let mut d = Downloads::default();
        d.enqueue("x/ep.mkv");
        let (id, _) = d.start_next().remove(0);
        assert!(d.progress(&id, 50, Some(200)));
        assert_eq!(d.get("x/ep.mkv").unwrap().progress(), 0.25);
        assert_eq!(d.overall(), 0.25);
        assert!(!d.progress("outro", 1, None));
    }

    #[test]
    fn limpar_concluidos() {
        let mut d = Downloads::default();
        d.enqueue("a");
        d.enqueue("b");
        d.items[0].status = DlStatus::Done;
        d.clear_finished();
        assert_eq!(d.items.len(), 1);
        assert_eq!(d.items[0].path, "a");
    }

    #[test]
    fn reenfileira_o_que_falhou() {
        let mut d = Downloads::default();
        d.enqueue("a");
        d.items[0].status = DlStatus::Error;
        d.enqueue("a");
        assert_eq!(d.items.len(), 1);
        assert_eq!(d.items[0].status, DlStatus::Queued);
    }

    #[test]
    fn cancelar_so_ativos() {
        let mut d = Downloads::default();
        d.enqueue("a");
        let id = d.items[0].id.clone();
        assert!(d.cancel(&id).is_some());
        assert!(d.items.is_empty());
        assert!(d.cancel(&id).is_none());
    }
}
