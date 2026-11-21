//! `PlayerSession`: regras do player sem interface — progresso, pular abertura,
//! próximo episódio com contagem, faixas lembradas e re-tentativas. Recebe o
//! estado do mpv e devolve `Action`s que a tela executa.

use std::time::{Duration, Instant};

use higanbana_core::library::join_path;
use higanbana_core::nuvem::FileEntry;
use higanbana_core::player::aniskip::SkipTimes;
use higanbana_core::player::logic::{
    can_auto_advance, classify_chapters, episode_number, estimate_ending, is_watched, next_card_start, next_episode, pick_tracks, retry_delay, AdvanceState, SubPick,
};
use higanbana_core::player::store::{PlayerData, Progress, Settings};
use higanbana_gpui::video::{PlayerState, Status};

use super::convert;
use super::target::PlayTarget;

/// Progresso gravado no máximo a cada tanto (e sempre ao pausar/sair/trocar).
const SAVE_EVERY: Duration = Duration::from_secs(5);

/// O que a tela precisa fazer (mpv, núcleo).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Abrir `path` (caminho na nuvem) começando em `start`.
    Load { path: String, size: u64, start: Option<f64> },
    SaveProgress { path: String, progress: Progress },
    /// `sid`/`aid` = id ou `no`.
    SetTrack { prop: &'static str, value: String },
    /// Sem capítulos de abertura: perguntar ao AniSkip e devolver em `set_skip`.
    ResolveSkip { file: String, mal: u32, ep: u32, dur: f64 },
}

pub struct PlayerSession {
    pub target: PlayTarget,
    pub data: PlayerData,
    next_file: Option<FileEntry>,
    playing_path: String,
    pos: f64,
    dur: f64,
    paused: bool,
    loaded: bool,
    skip: SkipTimes,
    skip_resolved_for: String,
    chapters_seen: usize,
    tracks_applied: bool,
    next_cancelled: bool,
    advanced_from: Option<String>,
    countdown: Option<f64>,
    last_tick: Instant,
    last_saved: Instant,
    errors_seen: u32,
    retry_attempt: usize,
    retry_at: Option<Instant>,
    net_error: bool,
    fatal: bool,
}

impl PlayerSession {
    pub fn new(target: PlayTarget, data: PlayerData, now: Instant) -> Self {
        let next_file = next_episode(&target.files, &target.file.name);
        PlayerSession {
            target,
            data,
            next_file,
            playing_path: String::new(),
            pos: 0.0,
            dur: 0.0,
            paused: false,
            loaded: false,
            skip: SkipTimes::default(),
            skip_resolved_for: String::new(),
            chapters_seen: 0,
            tracks_applied: false,
            next_cancelled: false,
            advanced_from: None,
            countdown: None,
            last_tick: now,
            last_saved: now,
            errors_seen: 0,
            retry_attempt: 0,
            retry_at: None,
            net_error: false,
            fatal: false,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.data.settings
    }

    pub fn next(&self) -> Option<&FileEntry> {
        self.next_file.as_ref()
    }

    pub fn pos(&self) -> f64 {
        self.pos
    }

    pub fn dur(&self) -> f64 {
        self.dur
    }

    pub fn net_error(&self) -> bool {
        self.net_error
    }

    /// Quando a próxima re-tentativa vence (para agendar o redesenho).
    pub fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }

    pub fn fatal(&self) -> bool {
        self.fatal
    }

    /// Caminho na nuvem do arquivo atual.
    pub fn remote_path(&self) -> String {
        join_path(&self.target.dir, &self.target.file.name)
    }

    /// Abre o arquivo atual do começo (ou de onde parou, se não terminou).
    pub fn start(&mut self) -> Action {
        self.load(None)
    }

    fn load(&mut self, start_at: Option<f64>) -> Action {
        self.playing_path = self.remote_path();
        let saved = self.data.progress.get(&self.playing_path).filter(|p| !p.watched).map(|p| p.position);
        let start = start_at.or(saved);
        self.loaded = false;
        self.tracks_applied = false;
        self.chapters_seen = 0;
        if start_at.is_none() {
            // Arquivo novo (não uma nova tentativa do mesmo).
            self.skip = SkipTimes::default();
            self.skip_resolved_for.clear();
            self.next_cancelled = false;
            self.fatal = false;
            self.retry_attempt = 0;
            self.pos = start.unwrap_or(0.0);
            self.dur = 0.0;
            self.countdown = None;
        }
        Action::Load { path: self.playing_path.clone(), size: self.target.file.size, start }
    }

    /// "Tentar de novo" da tela de erro: zera as tentativas e reabre na posição.
    pub fn retry(&mut self) -> Action {
        self.fatal = false;
        self.net_error = false;
        self.retry_attempt = 0;
        self.retry_at = None;
        self.load(Some(self.pos))
    }

    /// Estado novo do mpv.
    pub fn on_state(&mut self, st: &PlayerState, now: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        if st.end_error > self.errors_seen {
            self.errors_seen = st.end_error;
            self.on_error(now);
        }
        if let Some(at) = self.retry_at
            && now >= at
        {
            self.retry_at = None;
            out.push(self.load(Some(self.pos)));
            return out;
        }
        let loaded = matches!(st.status, Status::Playing | Status::Paused | Status::Ended);
        if loaded && !self.loaded {
            self.net_error = false;
            self.fatal = false;
            self.retry_attempt = 0;
        }
        self.loaded = loaded;
        if !loaded {
            return out;
        }
        self.dur = st.duration;
        self.pos = st.position;
        let paused = st.status == Status::Paused;
        let just_paused = paused && !self.paused;
        self.paused = paused;
        out.extend(self.record_progress(just_paused, now));
        out.extend(self.resolve_skip(st));
        out.extend(self.apply_tracks(st));
        out.extend(self.tick_countdown(now));
        let go = can_auto_advance(&AdvanceState {
            loaded,
            eof: st.status == Status::Ended,
            auto_next: self.settings().auto_next,
            cancelled: self.next_cancelled,
            has_next: self.next_file.is_some(),
            advanced_from: self.advanced_from.as_deref(),
            current: &self.target.file.name,
        });
        if go {
            out.extend(self.go_next(now));
        }
        out
    }

    /// Falhou: tenta de novo na mesma posição, esperando cada vez mais; desiste depois de 5.
    fn on_error(&mut self, now: Instant) {
        match retry_delay(self.retry_attempt) {
            Some(wait) => {
                self.retry_attempt += 1;
                self.net_error = true;
                self.retry_at = Some(now + wait);
            }
            None => {
                self.net_error = false;
                self.fatal = true;
                self.retry_at = None;
            }
        }
    }

    fn progress(&self) -> Progress {
        Progress {
            position: self.pos,
            duration: self.dur,
            watched: is_watched(self.pos, self.dur, self.skip.ed),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        }
    }

    fn record_progress(&mut self, force: bool, now: Instant) -> Option<Action> {
        if self.dur <= 0.0 || self.playing_path.is_empty() {
            return None;
        }
        let p = self.progress();
        self.data.progress.insert(self.playing_path.clone(), p.clone());
        if !force && now.saturating_duration_since(self.last_saved) < SAVE_EVERY {
            return None;
        }
        self.last_saved = now;
        Some(Action::SaveProgress { path: self.playing_path.clone(), progress: p })
    }

    /// Fechando o player: grava onde parou.
    pub fn close(&mut self, now: Instant) -> Option<Action> {
        self.record_progress(true, now)
    }

    /// Capítulos do arquivo (refeito se chegarem mais), senão AniSkip (só animes).
    fn resolve_skip(&mut self, st: &PlayerState) -> Option<Action> {
        if self.dur <= 0.0 {
            return None;
        }
        if st.chapters.len() > self.chapters_seen {
            self.chapters_seen = st.chapters.len();
            let found = classify_chapters(&convert::chapters(st), self.dur);
            if found.op.is_some() || found.ed.is_some() {
                self.skip = found;
                self.skip_resolved_for.clone_from(&self.target.file.name);
                return None;
            }
        }
        let name = &self.target.file.name;
        if self.skip_resolved_for == *name {
            return None;
        }
        self.skip_resolved_for.clone_from(name);
        let (Some(mal), true, Some(ep)) = (self.target.mal_id, self.target.is_anime, episode_number(name)) else { return None };
        Some(Action::ResolveSkip { file: name.clone(), mal, ep, dur: self.dur })
    }

    /// Resposta do AniSkip (ignorada se o arquivo já mudou).
    pub fn set_skip(&mut self, file: &str, times: SkipTimes) {
        if self.target.file.name == file {
            self.skip = times;
        }
    }

    /// Faixas lembradas do release ou idiomas preferidos, uma vez por carga.
    fn apply_tracks(&mut self, st: &PlayerState) -> Vec<Action> {
        if self.tracks_applied || (st.audio_tracks.is_empty() && st.sub_tracks.is_empty()) {
            return Vec::new();
        }
        self.tracks_applied = true;
        let s = &self.data.settings;
        let (sid, aid) = pick_tracks(&convert::tracks(st), self.data.tracks.get(&self.target.dir), &s.sub_lang, &s.audio_lang);
        let mut out = Vec::new();
        if let Some(sid) = sid {
            let value = match sid {
                SubPick::Id(id) => id.to_string(),
                SubPick::Off => "no".into(),
            };
            out.push(Action::SetTrack { prop: "sid", value });
        }
        if let Some(aid) = aid {
            out.push(Action::SetTrack { prop: "aid", value: aid.to_string() });
        }
        out
    }

    pub fn show_skip(&self) -> bool {
        self.settings().show_skip_intro && self.skip.op.is_some_and(|op| self.pos >= op.start && self.pos < op.end - 1.0)
    }

    /// Para onde pular (fim da abertura), se o botão está valendo.
    pub fn skip_intro(&self) -> Option<f64> {
        self.skip.op.filter(|_| self.show_skip()).map(|op| op.end)
    }

    /// Cartão do próximo: créditos (capítulos/AniSkip, senão estimativa), senão fim do arquivo.
    pub fn show_next(&self) -> bool {
        let ending = self.skip.ed.or_else(|| estimate_ending(self.dur, self.target.is_anime));
        self.next_file.is_some() && self.dur > 0.0 && !self.next_cancelled && self.pos >= next_card_start(self.dur, ending, self.settings().countdown_sec)
    }

    /// `(restante, total)` da contagem, se correndo.
    pub fn countdown(&self) -> Option<(f64, f64)> {
        self.countdown.map(|left| (left, self.settings().countdown_sec as f64))
    }

    fn tick_countdown(&mut self, now: Instant) -> Vec<Action> {
        let step = now.saturating_duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        if !(self.show_next() && self.settings().auto_next) {
            self.countdown = None;
            return Vec::new();
        }
        let Some(left) = self.countdown else {
            self.countdown = Some(self.settings().countdown_sec as f64);
            return Vec::new();
        };
        let left = if self.paused { left } else { (left - step).max(0.0) };
        self.countdown = Some(left);
        if left <= 0.0 { self.go_next(now) } else { Vec::new() }
    }

    pub fn cancel_next(&mut self) {
        self.next_cancelled = true;
        self.countdown = None;
    }

    /// Vai para o próximo episódio (uma vez por arquivo).
    pub fn go_next(&mut self, now: Instant) -> Vec<Action> {
        let Some(n) = self.next_file.clone() else { return Vec::new() };
        let from = self.target.file.name.clone();
        if self.advanced_from.as_deref() == Some(from.as_str()) {
            return Vec::new();
        }
        self.advanced_from = Some(from);
        self.switch_to(n, now)
    }

    /// Troca de episódio (lista de episódios ou próximo).
    pub fn switch_to(&mut self, file: FileEntry, now: Instant) -> Vec<Action> {
        let mut out: Vec<Action> = self.record_progress(true, now).into_iter().collect();
        self.target.file = file;
        self.next_file = next_episode(&self.target.files, &self.target.file.name);
        out.push(self.load(None));
        out
    }
}
