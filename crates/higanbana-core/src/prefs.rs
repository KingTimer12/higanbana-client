//! Preferências do usuário guardadas em disco. Por enquanto: a versão
//! (pasta de release) escolhida em cada obra.

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

pub struct Prefs {
    file: PathBuf,
    versions: Mutex<HashMap<String, String>>,
}

impl Prefs {
    pub fn open(file: PathBuf) -> Self {
        let versions = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { file, versions: Mutex::new(versions) }
    }

    /// Pasta de versão escolhida para a obra, se houver.
    pub fn version(&self, work_path: &str) -> Option<String> {
        self.versions.lock().unwrap().get(work_path).cloned()
    }

    /// Lembra a versão escolhida e grava em disco na hora.
    pub fn set_version(&self, work_path: &str, folder: &str) {
        let mut versions = self.versions.lock().unwrap();
        versions.insert(work_path.to_string(), folder.to_string());
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(&*versions) {
            let _ = std::fs::write(&self.file, bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_roundtrip() {
        let dir = std::env::temp_dir().join(format!("prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("prefs.json");
        let work = "Animes/Letra P/Precure/[2012.0] Smile Precure! [TV]";
        let prefs = Prefs::open(file.clone());
        assert_eq!(prefs.version(work), None);
        prefs.set_version(work, "[DragsterPS] Glitter Force …");
        let again = Prefs::open(file);
        assert_eq!(again.version(work).as_deref(), Some("[DragsterPS] Glitter Force …"));
        assert_eq!(again.version("Animes/Outra"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
