//! Cópias locais de arquivos da nuvem, para ver sem internet (o app mobile baixa
//! episódios e capítulos para cá). A cópia fica na pasta de dados do app, com um
//! nome que sai do caminho na nuvem: quem lê um arquivo (o leitor por faixas, o
//! player) pergunta [`copy_of`] antes de ir à rede.
//!
//! O manifesto (`offline/offline.json`) guarda o que a tela de baixados mostra
//! sem rede: a obra, o rótulo e o estado de cada arquivo.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::{sync::Semaphore, task::AbortHandle};

use crate::cloud::Cloud;

/// Downloads ao mesmo tempo: a banda do celular rende mais com poucos.
const PARALLEL: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum OfflineStatus {
    #[default]
    Queued,
    Downloading,
    Done,
    Failed,
}

/// Um arquivo baixado (ou na fila), com o que a tela precisa para abri-lo sem rede.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineItem {
    /// Caminho na nuvem: a identidade do arquivo.
    pub path: String,
    pub name: String,
    pub size: u64,
    /// `"anime"` ou `"manga"`.
    pub kind: String,
    /// Pasta da obra e o título que a tela mostra.
    pub work_path: String,
    pub work_title: String,
    /// `Ep. 3`, `Cap. 12`.
    pub label: String,
    #[serde(default)]
    pub added: i64,
    #[serde(default)]
    pub status: OfflineStatus,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    items: Vec<OfflineItem>,
}

pub struct Offline {
    dir: PathBuf,
    items: Mutex<Vec<OfflineItem>>,
    running: Mutex<HashMap<String, AbortHandle>>,
    slots: Semaphore,
}

/// Pasta das cópias dentro da pasta de dados.
fn dir_of(data: &Path) -> PathBuf {
    data.join("offline")
}

/// Hash estável (FNV-1a): o nome da cópia não pode mudar entre versões do app.
fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Onde fica a cópia de `remote` (exista ou não), com a extensão do original.
fn file_for(dir: &Path, remote: &str) -> PathBuf {
    let ext = remote
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "bin".to_owned());
    dir.join(format!("{:016x}.{ext}", stable_hash(remote)))
}

/// A cópia completa de `remote`, se existe. O arquivo só ganha o nome final ao
/// terminar (antes é `.part`), então existir basta.
pub fn copy_of(data: &Path, remote: &str) -> Option<PathBuf> {
    let file = file_for(&dir_of(data), remote);
    file.is_file().then_some(file)
}

impl Offline {
    pub fn open(data: &Path) -> Self {
        let dir = dir_of(data);
        let mut manifest: Manifest = std::fs::read(dir.join("offline.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        // O que estava baixando quando o app fechou não terminou: o `.part` já foi apagado.
        for item in &mut manifest.items {
            if item.status != OfflineStatus::Done {
                item.status = OfflineStatus::Failed;
                item.error = Some("interrompido".to_owned());
            } else if !file_for(&dir, &item.path).is_file() {
                item.status = OfflineStatus::Failed;
                item.error = Some("o arquivo sumiu".to_owned());
            }
        }
        Self { dir, items: Mutex::new(manifest.items), running: Mutex::default(), slots: Semaphore::new(PARALLEL) }
    }

    fn save(&self, items: &[OfflineItem]) {
        let _ = std::fs::create_dir_all(&self.dir);
        let manifest = Manifest { items: items.to_vec() };
        if let Ok(bytes) = serde_json::to_vec(&manifest) {
            let tmp = self.dir.join("offline.json.tmp");
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(tmp, self.dir.join("offline.json"));
            }
        }
    }

    fn update(&self, path: &str, f: impl FnOnce(&mut OfflineItem)) {
        let mut items = self.items.lock().unwrap();
        if let Some(item) = items.iter_mut().find(|i| i.path == path) {
            f(item);
            self.save(&items);
        }
    }

    /// Tudo que foi pedido, do mais recente.
    pub fn list(&self) -> Vec<OfflineItem> {
        let mut items = self.items.lock().unwrap().clone();
        items.sort_by_key(|i| std::cmp::Reverse(i.added));
        items
    }

    pub fn item(&self, path: &str) -> Option<OfflineItem> {
        self.items.lock().unwrap().iter().find(|i| i.path == path).cloned()
    }

    /// Bytes já baixados de `path` (o `.part` enquanto baixa; o tamanho todo quando terminou).
    pub fn downloaded(&self, path: &str) -> u64 {
        let file = file_for(&self.dir, path);
        let part = PathBuf::from(format!("{}.part", file.display()));
        std::fs::metadata(&file).or_else(|_| std::fs::metadata(part)).map(|m| m.len()).unwrap_or(0)
    }

    /// A cópia local de `path`, quando já terminou.
    pub fn local(&self, path: &str) -> Option<PathBuf> {
        copy_of(self.dir.parent()?, path)
    }

    /// Põe o arquivo na fila (ou de volta, se falhou). `false` se já está baixando ou baixado.
    fn enqueue(&self, mut item: OfflineItem) -> bool {
        let mut items = self.items.lock().unwrap();
        match items.iter_mut().find(|i| i.path == item.path) {
            Some(i) if i.status != OfflineStatus::Failed => return false,
            Some(i) => {
                i.status = OfflineStatus::Queued;
                i.error = None;
            }
            None => {
                item.status = OfflineStatus::Queued;
                item.error = None;
                item.added = crate::auth::session::now();
                items.push(item);
            }
        }
        self.save(&items);
        true
    }

    /// Baixa `item` em segundo plano, no runtime de `rt`.
    pub fn start(self: &Arc<Self>, rt: &tokio::runtime::Handle, cloud: Arc<Cloud>, item: OfflineItem) {
        let path = item.path.clone();
        if !self.enqueue(item) {
            return;
        }
        let me = self.clone();
        let task = rt.spawn({
            let path = path.clone();
            async move { me.run(&cloud, &path).await }
        });
        self.running.lock().unwrap().insert(path, task.abort_handle());
    }

    async fn run(&self, cloud: &Cloud, path: &str) {
        let Ok(_slot) = self.slots.acquire().await else { return };
        self.update(path, |i| i.status = OfflineStatus::Downloading);
        let dest = file_for(&self.dir, path);
        let id = format!("offline-{:016x}", stable_hash(path));
        let result = cloud.download(&id, path, Some(dest)).await;
        self.update(path, |i| match result {
            Ok(_) => i.status = OfflineStatus::Done,
            Err(e) => {
                i.status = OfflineStatus::Failed;
                i.error = Some(e.to_string());
            }
        });
        self.running.lock().unwrap().remove(path);
    }

    /// Interrompe (se baixando), apaga a cópia e tira da lista.
    pub fn remove(&self, path: &str) {
        if let Some(task) = self.running.lock().unwrap().remove(path) {
            // O guarda do download apaga o `.part` ao ser abortado.
            task.abort();
        }
        let file = file_for(&self.dir, path);
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(format!("{}.part", file.display()));
        let mut items = self.items.lock().unwrap();
        items.retain(|i| i.path != path);
        self.save(&items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str) -> OfflineItem {
        OfflineItem {
            path: path.to_owned(),
            name: "Ep 01.mkv".to_owned(),
            size: 10,
            kind: "anime".to_owned(),
            work_path: "Animes/x".to_owned(),
            work_title: "X".to_owned(),
            label: "Ep. 1".to_owned(),
            added: 0,
            status: OfflineStatus::Queued,
            error: None,
        }
    }

    #[test]
    fn nome_da_copia_e_estavel() {
        let dir = Path::new("/d");
        assert_eq!(file_for(dir, "Animes/x/Ep 01.MKV"), file_for(dir, "Animes/x/Ep 01.MKV"));
        assert!(file_for(dir, "Animes/x/Ep 01.MKV").to_string_lossy().ends_with(".mkv"));
        assert_ne!(file_for(dir, "a.cbz"), file_for(dir, "b.cbz"));
        assert!(file_for(dir, "sem-extensao").to_string_lossy().ends_with(".bin"));
        // FNV-1a de "a": o valor não pode mudar entre versões.
        assert_eq!(stable_hash("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn manifesto_sobrevive_e_marca_o_interrompido() {
        let data = std::env::temp_dir().join(format!("higanbana-offline-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let offline = Offline::open(&data);
        assert!(offline.enqueue(item("Animes/x/Ep 01.mkv")));
        assert!(!offline.enqueue(item("Animes/x/Ep 01.mkv")), "já na fila");
        // Um terminado: a cópia existe com o nome final.
        assert!(offline.enqueue(item("Animes/x/Ep 02.mkv")));
        let done = file_for(&dir_of(&data), "Animes/x/Ep 02.mkv");
        std::fs::write(&done, b"0123456789").unwrap();
        offline.update("Animes/x/Ep 02.mkv", |i| i.status = OfflineStatus::Done);

        let again = Offline::open(&data);
        assert_eq!(again.item("Animes/x/Ep 01.mkv").unwrap().status, OfflineStatus::Failed);
        assert_eq!(again.item("Animes/x/Ep 02.mkv").unwrap().status, OfflineStatus::Done);
        assert_eq!(copy_of(&data, "Animes/x/Ep 02.mkv"), Some(done.clone()));
        assert_eq!(again.downloaded("Animes/x/Ep 02.mkv"), 10);
        // O que falhou pode voltar à fila.
        assert!(again.enqueue(item("Animes/x/Ep 01.mkv")));

        again.remove("Animes/x/Ep 02.mkv");
        assert!(!done.exists());
        assert!(again.item("Animes/x/Ep 02.mkv").is_none());
        let _ = std::fs::remove_dir_all(&data);
    }
}
