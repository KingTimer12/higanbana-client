//! Opções do mpv passadas antes do `mpv_initialize`.

/// Opções padrão do player com render API.
pub const DEFAULT_OPTIONS: &[(&str, &str)] = &[
    ("vo", "libmpv"),
    ("hwdec", "auto-copy"),
    ("keep-open", "yes"),
    ("idle", "yes"),
    ("terminal", "no"),
    ("input-default-bindings", "no"),
    ("osc", "no"),
    ("ytdl", "no"),
    ("sub-auto", "fuzzy"),
];

/// `PlayerOptions::default().set("cache", "yes")`; chave repetida sobrescreve.
#[derive(Clone, Debug)]
pub struct PlayerOptions {
    items: Vec<(String, String)>,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        PlayerOptions { items: DEFAULT_OPTIONS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }
}

impl PlayerOptions {
    /// Sem nenhuma opção (ex.: player headless).
    pub fn empty() -> Self {
        PlayerOptions { items: Vec::new() }
    }

    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let (key, value) = (key.into(), value.into());
        match self.items.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.items.push((key, value)),
        }
        self
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.items.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
