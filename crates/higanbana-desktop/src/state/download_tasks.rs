//! Dispara e acompanha os downloads da fila no núcleo.

use higanbana_core::nuvem::download::DownloadProgress;

use super::{AppState, DlStatus, Tone};

impl AppState {
    pub fn start_downloads(&mut self, paths: &[String]) {
        for p in paths {
            self.downloads.enqueue(p);
        }
        self.pump_downloads();
    }

    fn pump_downloads(&mut self) {
        for (id, path) in self.downloads.start_next() {
            let core = self.core.clone();
            let (i, p, key) = (id.clone(), path, id.clone());
            let handle = self.run(async move { core.cloud().download(&i, &p, None).await.map_err(|e| e.to_string()) }, move |s, result, _| {
                s.download_finished(&id, result)
            });
            if let Some(d) = self.downloads.by_id(&key) {
                d.abort = Some(handle.abort_handle());
            }
        }
    }

    fn download_finished(&mut self, id: &str, result: Result<std::path::PathBuf, String>) {
        // Cancelado: a linha já saiu da lista e não há o que avisar.
        let Some(d) = self.downloads.by_id(id) else { return };
        d.abort = None;
        let name = d.name.clone();
        match result {
            Ok(dest) => {
                d.status = DlStatus::Done;
                d.dest = Some(dest);
                self.toast(format!("{name} baixado"), Tone::Ok);
            }
            Err(e) => {
                d.status = DlStatus::Error;
                d.error = Some(e);
                self.toast("Um download falhou. Abra Downloads para tentar de novo.", Tone::Error);
            }
        }
        self.pump_downloads();
    }

    pub fn on_download_progress(&mut self, p: &DownloadProgress) {
        self.downloads.progress(&p.id, p.downloaded, p.total);
    }

    /// Tira da fila ou interrompe (o núcleo apaga o `.part`) e começa o próximo.
    pub fn cancel_download(&mut self, id: &str) {
        if let Some(Some(abort)) = self.downloads.cancel(id) {
            abort.abort();
        }
        self.pump_downloads();
    }

    pub fn clear_finished(&mut self) {
        self.downloads.clear_finished();
    }
}
