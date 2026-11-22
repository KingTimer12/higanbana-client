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

fn gpu_default() -> bool {
    std::env::var("HIGANBANA_VIDEO_GPU").map_or(true, |v| v != "0")
}

/// `PlayerOptions::default().set("cache", "yes")`; chave repetida sobrescreve.
#[derive(Clone, Debug)]
pub struct PlayerOptions {
    items: Vec<(String, String)>,
    /// Desenhar na GPU quando der (D3D11). `HIGANBANA_VIDEO_GPU=0` desliga no padrão.
    gpu: bool,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        PlayerOptions { items: DEFAULT_OPTIONS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), gpu: gpu_default() }
    }
}

impl PlayerOptions {
    /// Sem nenhuma opção (ex.: player headless).
    pub fn empty() -> Self {
        PlayerOptions { items: Vec::new(), gpu: gpu_default() }
    }

    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let (key, value) = (key.into(), value.into());
        match self.items.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.items.push((key, value)),
        }
        self
    }

    /// Liga/desliga o vídeo na GPU (o padrão segue `HIGANBANA_VIDEO_GPU`).
    pub fn gpu(mut self, on: bool) -> Self {
        self.gpu = on;
        self
    }

    pub fn uses_gpu(&self) -> bool {
        self.gpu
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.items.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
