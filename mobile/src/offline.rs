//! Baixados para ver sem internet (`Offline` em `higanbana/native`): o núcleo
//! guarda a cópia na pasta do app, e o player e o leitor a usam no lugar da nuvem.

use higanbana_core::offline::{OfflineItem, OfflineStatus};
use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::core;

/// Downloads de episódios e capítulos.
#[module]
pub struct Offline;

#[module]
impl Offline {
    /// Baixa um arquivo da nuvem: `{ path, name, size, kind, workPath, workTitle, label }`.
    /// Já baixando ou baixado, não faz nada; se falhou, tenta de novo.
    pub fn download(item: JsonValue) -> Result<(), String> {
        let item: OfflineItem = serde_json::from_value(item).map_err(|e| e.to_string())?;
        core()?.download_offline(item);
        Ok(())
    }

    /// Tudo que foi pedido, do mais recente:
    /// `[{ path, name, size, kind, workPath, workTitle, label, status, error, downloaded }]`,
    /// com `status` em `queued`, `downloading`, `done` ou `failed`.
    pub fn list() -> Result<JsonValue, String> {
        let c = core()?;
        let offline = c.offline();
        Ok(offline
            .list()
            .into_iter()
            .map(|i| {
                let downloaded = match i.status {
                    OfflineStatus::Done => i.size,
                    OfflineStatus::Downloading => offline.downloaded(&i.path),
                    _ => 0,
                };
                let mut v = serde_json::to_value(&i).unwrap_or(JsonValue::Null);
                v["downloaded"] = json!(downloaded);
                v
            })
            .collect())
    }

    /// Interrompe, apaga a cópia e tira da lista.
    pub fn remove(path: String) -> Result<(), String> {
        core()?.offline().remove(&path);
        Ok(())
    }
}
