//! Estado observado do mpv (tempo, pausa, volume, faixas) atualizado pela thread de eventos.
use super::api::*;
use std::ffi::{c_char, c_int, CStr};

/// Situação da reprodução.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Status {
    /// Nada carregado.
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    /// Chegou ao fim (o último quadro fica na tela).
    Ended,
    Error(String),
}

/// Faixa de áudio ou legenda.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub lang: String,
}

impl Track {
    /// Nome para menus: título, idioma ou "Faixa N".
    pub fn label(&self) -> String {
        match (self.title.is_empty(), self.lang.is_empty()) {
            (false, false) => format!("{} ({})", self.title, self.lang),
            (false, true) => self.title.clone(),
            (true, false) => self.lang.clone(),
            (true, true) => format!("Faixa {}", self.id),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub status: Status,
    /// Segundos.
    pub position: f64,
    pub duration: f64,
    /// 0..=100.
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub audio_tracks: Vec<Track>,
    pub sub_tracks: Vec<Track>,
    pub audio: Option<i64>,
    pub sub: Option<i64>,
    pub video_size: Option<(u32, u32)>,
    /// Interno: valor bruto de `pause` (o status depende também de carregamento/fim).
    pub(crate) paused: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            status: Status::Idle,
            position: 0.0,
            duration: 0.0,
            volume: 100.0,
            muted: false,
            speed: 1.0,
            audio_tracks: Vec::new(),
            sub_tracks: Vec::new(),
            audio: None,
            sub: None,
            video_size: None,
            paused: false,
        }
    }
}

/// Propriedades observadas e o formato pedido ao mpv.
pub(crate) const OBSERVED: &[(&str, c_int)] = &[
    ("time-pos", FORMAT_DOUBLE),
    ("duration", FORMAT_DOUBLE),
    ("pause", FORMAT_FLAG),
    ("volume", FORMAT_DOUBLE),
    ("mute", FORMAT_FLAG),
    ("speed", FORMAT_DOUBLE),
    ("eof-reached", FORMAT_FLAG),
    ("dwidth", FORMAT_INT64),
    ("dheight", FORMAT_INT64),
    ("aid", FORMAT_STRING),
    ("sid", FORMAT_STRING),
    ("track-list", FORMAT_NODE),
];

unsafe fn text(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

/// Itens de um `mpv_node` lista/mapa.
unsafe fn entries(node: &Node) -> Vec<(Option<String>, &Node)> {
    if node.format != FORMAT_NODE_ARRAY && node.format != FORMAT_NODE_MAP {
        return Vec::new();
    }
    let list = unsafe { &*node.u.list };
    let n = list.num.max(0) as usize;
    if n == 0 || list.values.is_null() {
        return Vec::new();
    }
    let values = unsafe { std::slice::from_raw_parts(list.values, n) };
    let keys = if node.format == FORMAT_NODE_MAP && !list.keys.is_null() {
        unsafe { std::slice::from_raw_parts(list.keys, n) }.iter().map(|k| Some(unsafe { text(*k) })).collect()
    } else {
        vec![None; n]
    };
    keys.into_iter().zip(values).collect()
}

/// `track-list` → faixas de áudio e legenda.
unsafe fn tracks(node: &Node) -> (Vec<Track>, Vec<Track>) {
    let (mut audio, mut subs) = (Vec::new(), Vec::new());
    for (_, item) in unsafe { entries(node) } {
        let (mut kind, mut t) = (String::new(), Track { id: 0, title: String::new(), lang: String::new() });
        for (k, v) in unsafe { entries(item) } {
            match (k.as_deref(), v.format) {
                (Some("type"), FORMAT_STRING) => kind = unsafe { text(v.u.string) },
                (Some("id"), FORMAT_INT64) => t.id = unsafe { v.u.int64 },
                (Some("title"), FORMAT_STRING) => t.title = unsafe { text(v.u.string) },
                (Some("lang"), FORMAT_STRING) => t.lang = unsafe { text(v.u.string) },
                _ => {}
            }
        }
        match kind.as_str() {
            "audio" => audio.push(t),
            "sub" => subs.push(t),
            _ => {}
        }
    }
    (audio, subs)
}

impl PlayerState {
    /// Aplica uma mudança de propriedade vinda de `MPV_EVENT_PROPERTY_CHANGE`.
    ///
    /// # Safety
    /// `p` vem do mpv e é válido durante o evento.
    pub(crate) unsafe fn apply(&mut self, p: &RawProperty) {
        let name = unsafe { text(p.name) };
        let none = p.data.is_null();
        let double = || if none { 0.0 } else { unsafe { *(p.data as *const f64) } };
        let flag = || !none && unsafe { *(p.data as *const c_int) } != 0;
        let int = || if none { 0 } else { unsafe { *(p.data as *const i64) } };
        let string = || if none { String::new() } else { unsafe { text(*(p.data as *const *const c_char)) } };
        match name.as_str() {
            "time-pos" => self.position = double(),
            "duration" => self.duration = double(),
            "volume" => self.volume = double(),
            "speed" => self.speed = if none { 1.0 } else { double() },
            "mute" => self.muted = flag(),
            "pause" => {
                self.paused = flag();
                if matches!(self.status, Status::Playing | Status::Paused) {
                    self.status = if self.paused { Status::Paused } else { Status::Playing };
                }
            }
            "eof-reached" => {
                if flag() {
                    self.status = Status::Ended;
                } else if self.status == Status::Ended {
                    self.status = if self.paused { Status::Paused } else { Status::Playing };
                }
            }
            "dwidth" | "dheight" => {
                let v = int();
                let (w, h) = self.video_size.unwrap_or((0, 0));
                let size = if name == "dwidth" { (v as u32, h) } else { (w, v as u32) };
                self.video_size = (v > 0).then_some(size);
            }
            "aid" => self.audio = string().parse().ok(),
            "sid" => self.sub = string().parse().ok(),
            "track-list" if !none && p.format == FORMAT_NODE => {
                (self.audio_tracks, self.sub_tracks) = unsafe { tracks(&*(p.data as *const Node)) };
            }
            _ => {}
        }
    }

    pub(crate) fn file_loaded(&mut self) {
        self.status = if self.paused { Status::Paused } else { Status::Playing };
    }
}
