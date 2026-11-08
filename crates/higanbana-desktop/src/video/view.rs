//! Tela do player ("Imersivo"): o mpv desenha na janela filha embaixo e esta
//! tela é só a interface por cima.

use crate::interact::{hover_layer, radios};
use crate::interact::Button;
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::StreamExt;
use gpui_kit::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    AnimationExt as _, Pixels, Render, SharedString, Stateful, Styled, Subscription, Task, Window, assets::IconName, canvas, div,
    ObjectFit, img, linear_color_stop, linear_gradient, prelude::*, px, relative, rems,
};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex,
    button::ButtonVariants as _,
    popover::Popover,
    progress::Progress as ProgressBar,
    radio::RadioGroup,
    separator::Separator as Divider,
    slider::{Slider, SliderEvent, SliderState},
    spinner::Spinner,
    switch::Switch,
};
use higanbana_core::{
    Core,
    library::{join_path, parse_episode},
    nuvem::FileEntry,
    player::{
        aniskip::SkipTimes,
        logic::{
            AdvanceState, Chapter, SubPick, Track, TrackKind, can_auto_advance, classify_chapters, episode_label,
            episode_number, estimate_ending, format_time, is_watched, next_card_start, next_episode, pick_tracks,
            retry_delay, video_files,
        },
        store::{PlayerData, Progress, Settings, TrackPref},
    },
};
use serde_json::Value;

use super::{
    mpv::{EndReason, Mpv, MpvEvent, Observe},
    surface::Surface,
    thumbs::{self, Thumbs},
};
use crate::{
    theme::{self, ProductColors as _},
    ui::truncate,
};

/// Tamanho da prévia do quadro sobre a barra de posição (16:9).
const THUMB_W: Pixels = px(176.);
const THUMB_H: Pixels = px(99.);
const HIDE_AFTER: Duration = Duration::from_millis(2500);
/// Progresso gravado em disco no máximo a cada tanto (e sempre ao pausar/sair/trocar).
const SAVE_EVERY: Duration = Duration::from_secs(5);

/// O que está tocando. `dir` é a pasta do release (chave das faixas lembradas).
#[derive(Debug, Clone)]
pub struct PlayTarget {
    pub dir: String,
    pub file: FileEntry,
    pub files: Vec<FileEntry>,
    pub mal_id: Option<u32>,
    /// Seção `Animes/` (consulta o AniSkip); doramas e desenhos não.
    pub is_anime: bool,
    pub anime_title: String,
}

pub enum PlayerEvent {
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Tracks,
    Settings,
    Episodes,
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Seek(f64),
}

/// Opções fixas: nada da interface do próprio mpv, cache só em RAM.
fn base_options(wid: usize, cache_mib: u32) -> Vec<(&'static str, String)> {
    vec![
        ("wid", wid.to_string()),
        ("vo", "gpu-next".into()),
        ("hwdec", "auto-safe".into()),
        ("keep-open", "yes".into()),
        ("force-window", "yes".into()),
        ("osc", "no".into()),
        ("osd-level", "0".into()),
        ("input-default-bindings", "no".into()),
        ("input-vo-keyboard", "no".into()),
        ("input-cursor", "no".into()),
        ("cursor-autohide", "no".into()),
        ("cache", "yes".into()),
        ("cache-on-disk", "no".into()),
        ("demuxer-max-bytes", format!("{cache_mib}MiB")),
        ("demuxer-max-back-bytes", "50MiB".into()),
        ("sub-auto", "no".into()),
    ]
}

const OBSERVED: [(&str, Observe); 11] = [
    // Saída de vídeo pronta: antes disso a janela do mpv ainda não tem quadro.
    ("vo-configured", Observe::Flag),
    ("time-pos", Observe::Double),
    ("duration", Observe::Double),
    ("pause", Observe::Flag),
    ("volume", Observe::Double),
    ("mute", Observe::Flag),
    ("paused-for-cache", Observe::Flag),
    ("demuxer-cache-time", Observe::Double),
    ("chapter-list", Observe::Node),
    ("track-list", Observe::Node),
    ("eof-reached", Observe::Flag),
];

pub struct PlayerView {
    core: Arc<Core>,
    target: PlayTarget,
    surface: Rc<Surface>,
    mpv: Option<Mpv>,
    focus: FocusHandle,
    data: PlayerData,

    pos: f64,
    dur: f64,
    paused: bool,
    volume: f64,
    muted: bool,
    buffering: bool,
    cache_ahead: f64,
    chapters: Vec<Chapter>,
    tracks: Vec<Track>,
    eof: bool,
    menu: Option<Menu>,
    idle: bool,
    net_error: bool,
    fatal: bool,
    /// Falha ao iniciar o mpv (DLL ausente etc.).
    broken: Option<SharedString>,
    skip: SkipTimes,
    next_cancelled: bool,
    /// Arquivo atual aberto pelo mpv (`file-loaded`). Antes disso, eventos podem ser do anterior.
    loaded: bool,
    /// O mpv já tem saída de vídeo (`vo-configured`).
    vo_configured: bool,
    /// Chegou posição do arquivo atual: o quadro dele já está na tela.
    playing: bool,
    expected_chapters: usize,
    skip_resolved_for: String,
    load_seq: u64,
    tracks_applied_for: u64,
    /// Arquivo de onde já saímos para o próximo: evita avançar duas vezes.
    advanced_from: Option<String>,
    playing_path: String,
    retry_attempt: usize,
    retry_task: Option<Task<()>>,
    hide_task: Option<Task<()>>,
    /// Segundos restantes da contagem do próximo episódio.
    countdown: Option<f64>,
    countdown_task: Option<Task<()>>,
    last_saved: Instant,
    drag: Option<Drag>,
    seek_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Volume do mpv (0..100) no `Slider` do kit.
    volume_slider: Entity<SliderState>,
    hover: Option<(Pixels, f64)>,
    /// Miniaturas da barra de posição: abertas no primeiro hover de cada arquivo.
    thumbs: Option<Thumbs>,
    _thumb_events: Option<Task<()>>,
    /// Última posição do mouse (o GPUI repete o evento a cada quadro).
    last_mouse: Option<gpui_kit::Point<Pixels>>,
    _events: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PlayerEvent> for PlayerView {}

impl Focusable for PlayerView {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PlayerView {
    pub fn new(core: Arc<Core>, target: PlayTarget, surface: Rc<Surface>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let data = core.player().data();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let volume_slider = cx.new(|_| SliderState::new().min(0.).max(100.).step(1.).default_value(100.));
        let subs = vec![cx.subscribe(&volume_slider, |v, _, e: &SliderEvent, cx| {
            let (SliderEvent::Change(value) | SliderEvent::Release(value)) = e;
            v.set_volume(value.start() as f64);
            v.poke(cx);
            cx.notify();
        })];
        let mut this = Self {
            core,
            target,
            surface,
            mpv: None,
            focus,
            data,
            pos: 0.,
            dur: 0.,
            paused: false,
            volume: 100.,
            muted: false,
            buffering: true,
            cache_ahead: 0.,
            chapters: Vec::new(),
            tracks: Vec::new(),
            eof: false,
            menu: None,
            idle: false,
            net_error: false,
            fatal: false,
            broken: None,
            skip: SkipTimes::default(),
            next_cancelled: false,
            loaded: false,
            vo_configured: false,
            playing: false,
            expected_chapters: 0,
            skip_resolved_for: String::new(),
            load_seq: 0,
            tracks_applied_for: u64::MAX,
            advanced_from: None,
            playing_path: String::new(),
            retry_attempt: 0,
            retry_task: None,
            hide_task: None,
            countdown: None,
            countdown_task: None,
            last_saved: Instant::now(),
            drag: None,
            seek_bounds: Rc::default(),
            volume_slider,
            hover: None,
            thumbs: None,
            _thumb_events: None,
            last_mouse: None,
            _events: None,
            _subs: subs,
        };
        match Mpv::new(&base_options(this.surface.wid(), this.data.settings.cache_mib), &OBSERVED) {
            Ok((mpv, mut rx)) => {
                this.mpv = Some(mpv);
                this._events = Some(cx.spawn_in(window, async move |this, cx| {
                    while let Some(e) = rx.next().await {
                        let r = this.update_in(cx, |v, window, cx| {
                            v.on_mpv(e, cx);
                            v.sync_volume_slider(window, cx);
                        });
                        if r.is_err() {
                            break;
                        }
                    }
                }));
                this.load_current(None, cx);
            }
            Err(e) => this.broken = Some(e.into()),
        }
        this.poke(cx);
        this
    }

    fn file(&self) -> &FileEntry {
        &self.target.file
    }

    fn remote_path(&self) -> String {
        join_path(&self.target.dir, &self.file().name)
    }

    fn next(&self) -> Option<FileEntry> {
        next_episode(&self.target.files, &self.file().name)
    }

    fn settings(&self) -> &Settings {
        &self.data.settings
    }

    // --- Progresso ---

    fn record_progress(&mut self, force: bool) {
        if self.dur <= 0. || self.playing_path.is_empty() {
            return;
        }
        let p = Progress {
            position: self.pos,
            duration: self.dur,
            watched: is_watched(self.pos, self.dur, self.skip.ed),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        };
        self.data.progress.insert(self.playing_path.clone(), p.clone());
        if force || self.last_saved.elapsed() >= SAVE_EVERY {
            self.last_saved = Instant::now();
            self.core.player().save_progress(self.playing_path.clone(), p);
        }
    }

    // --- mpv ---

    fn on_mpv(&mut self, e: MpvEvent, cx: &mut Context<Self>) {
        match e {
            MpvEvent::Prop(name, v) => self.on_prop(&name, v),
            MpvEvent::FileLoaded => {
                self.expected_chapters = 0;
                self.loaded = true;
                self.net_error = false;
                self.fatal = false;
                self.retry_attempt = 0;
            }
            MpvEvent::EndFile(EndReason::Error) => self.on_error(cx),
            MpvEvent::EndFile(_) | MpvEvent::Restart => {}
        }
        self.effects(cx);
        cx.notify();
    }

    fn on_prop(&mut self, name: &str, v: Value) {
        let num = v.as_f64();
        let flag = v.as_bool().unwrap_or(false);
        match name {
            "time-pos" => {
                if let (Some(t), true) = (num, self.loaded) {
                    self.pos = t;
                    self.playing = true;
                    self.record_progress(false);
                }
            }
            "duration" => self.dur = num.unwrap_or(0.),
            "pause" => {
                self.paused = flag;
                if flag {
                    self.record_progress(true);
                }
            }
            "volume" => self.volume = num.unwrap_or(100.).round(),
            "mute" => self.muted = flag,
            "paused-for-cache" => self.buffering = flag,
            "vo-configured" => self.vo_configured = flag,
            "demuxer-cache-time" => self.cache_ahead = num.unwrap_or(0.),
            "chapter-list" => {
                self.chapters = serde_json::from_value(v).unwrap_or_default();
                self.expected_chapters = self.expected_chapters.max(self.chapters.len());
            }
            "track-list" => self.tracks = serde_json::from_value(v).unwrap_or_default(),
            "eof-reached"
                if self.loaded => {
                    self.eof = flag;
                    if flag {
                        self.idle = false;
                    }
                }
            _ => {}
        }
    }

    /// Pede a miniatura do tempo `t`, abrindo o gerador no primeiro pedido do arquivo.
    fn want_thumb(&mut self, t: f64, cx: &mut Context<Self>) {
        if !self.loaded || self.broken.is_some() {
            return;
        }
        if self.thumbs.is_none() {
            let seq = self.load_seq;
            let url = self.core.player().stream_url(&self.playing_path, self.file().size);
            let dir = std::env::temp_dir().join("higanbana-thumbs").join(format!("{}-{seq}", std::process::id()));
            let Ok((thumbs, mut rx)) = Thumbs::open(&url, dir) else { return };
            self.thumbs = Some(thumbs);
            self._thumb_events = Some(cx.spawn(async move |this, cx| {
                while let Some(e) = rx.next().await {
                    let Ok(shot) = this.update(cx, |v, _| v.thumbs.as_mut().filter(|_| v.load_seq == seq).and_then(|t| t.on_event(e))) else { break };
                    let Some(shot) = shot else { continue };
                    let bucket = shot.bucket;
                    // Gravar o JPEG bloqueia: fora da thread da interface.
                    let ok = cx.background_executor().spawn(async move { shot.run() }).await;
                    let r = this.update(cx, |v, cx| {
                        if let Some(t) = v.thumbs.as_mut().filter(|_| v.load_seq == seq) {
                            t.finish(bucket, ok);
                            cx.notify();
                        }
                    });
                    if r.is_err() {
                        break;
                    }
                }
            }));
        }
        if let Some(th) = &mut self.thumbs {
            th.want(thumbs::bucket(t));
        }
    }

    /// Falhou: tenta de novo na mesma posição, esperando cada vez mais, e desiste depois de 5.
    fn on_error(&mut self, cx: &mut Context<Self>) {
        let Some(wait) = retry_delay(self.retry_attempt) else {
            self.net_error = false;
            self.buffering = false;
            self.fatal = true;
            return;
        };
        self.retry_attempt += 1;
        self.net_error = true;
        self.retry_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |v, cx| {
                let at = v.pos;
                v.load_current(Some(at), cx);
            });
        }));
    }

    fn load_current(&mut self, start_at: Option<f64>, cx: &mut Context<Self>) {
        let Some(mpv) = &self.mpv else { return };
        self.playing_path = self.remote_path();
        let saved = self.data.progress.get(&self.playing_path).filter(|p| !p.watched).map(|p| p.position);
        let start = start_at.or(saved).unwrap_or(0.);
        self.load_seq += 1;
        self.loaded = false;
        self.playing = false;
        // Miniaturas são do arquivo anterior (ou de uma tentativa que falhou).
        self.thumbs = None;
        self._thumb_events = None;
        self.expected_chapters = 0;
        self.buffering = true;
        self.eof = false;
        self.chapters.clear();
        self.tracks.clear();
        if start_at.is_none() {
            // Arquivo novo (não uma nova tentativa do mesmo).
            self.skip = SkipTimes::default();
            self.next_cancelled = false;
            self.fatal = false;
            self.skip_resolved_for.clear();
            self.retry_attempt = 0;
            self.pos = start;
            self.dur = 0.;
            self.countdown = None;
            self.countdown_task = None;
        }
        let url = self.core.player().stream_url(&self.playing_path, self.file().size);
        let mut args = vec!["loadfile".to_owned(), url, "replace".into(), "-1".into()];
        if start > 1. {
            args.push(format!("start={start:.1}"));
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if mpv.command(&args).is_err() {
            self.on_error(cx);
        }
        cx.notify();
    }

    /// Reações ao estado: abertura/encerramento, faixas, próximo episódio.
    fn effects(&mut self, cx: &mut Context<Self>) {
        self.resolve_skip(cx);
        self.apply_tracks();
        self.update_countdown(cx);
        let go = can_auto_advance(&AdvanceState {
            loaded: self.loaded,
            eof: self.eof,
            auto_next: self.settings().auto_next,
            cancelled: self.next_cancelled,
            has_next: self.next().is_some(),
            advanced_from: self.advanced_from.as_deref(),
            current: &self.target.file.name,
        });
        if go {
            self.go_next(cx);
        }
    }

    /// Capítulos do arquivo, senão AniSkip (só animes). Uma vez por arquivo.
    fn resolve_skip(&mut self, cx: &mut Context<Self>) {
        let name = self.file().name.clone();
        if !self.loaded || self.dur <= 0. || self.skip_resolved_for == name || self.chapters.len() < self.expected_chapters {
            return;
        }
        self.skip_resolved_for = name.clone();
        let from_chapters = classify_chapters(&self.chapters, self.dur);
        if from_chapters.op.is_some() || from_chapters.ed.is_some() {
            self.skip = from_chapters;
            return;
        }
        let (Some(mal), true, Some(ep)) = (self.target.mal_id, self.target.is_anime, episode_number(&name)) else { return };
        let core = self.core.clone();
        let dur = self.dur;
        let task = self.core.spawn(async move { core.player().aniskip(mal, ep, dur).await });
        cx.spawn(async move |this, cx| {
            if let Ok(times) = task.await {
                let _ = this.update(cx, |v, cx| {
                    if v.file().name == name {
                        v.skip = times;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// Faixas lembradas do release ou idiomas preferidos, a cada carga.
    fn apply_tracks(&mut self) {
        if !self.loaded || !self.tracks.iter().any(|t| t.kind != TrackKind::Video) || self.tracks_applied_for == self.load_seq {
            return;
        }
        self.tracks_applied_for = self.load_seq;
        let s = &self.data.settings;
        let (sid, aid) = pick_tracks(&self.tracks, self.data.tracks.get(&self.target.dir), &s.sub_lang, &s.audio_lang);
        if let Some(sid) = sid {
            self.set_track("sid", sid);
        }
        if let Some(aid) = aid {
            self.set_track("aid", SubPick::Id(aid));
        }
    }

    fn set_track(&self, prop: &str, pick: SubPick) {
        let value = match pick {
            SubPick::Id(id) => id.to_string(),
            SubPick::Off => "no".into(),
        };
        if let Some(m) = &self.mpv {
            let _ = m.set(prop, &value);
        }
    }

    // --- Ações ---

    fn toggle_pause(&mut self) {
        if let Some(m) = &self.mpv {
            let _ = m.set_flag("pause", !self.paused);
        }
    }

    fn seek(&mut self, t: f64) {
        let t = t.min(self.dur - 0.5).max(0.);
        if let Some(m) = &self.mpv {
            let _ = m.command(&["seek", &format!("{t:.3}"), "absolute+exact"]);
        }
        self.pos = t;
    }

    fn set_volume(&mut self, v: f64) {
        let v = v.clamp(0., 100.);
        if let Some(m) = &self.mpv {
            let _ = m.set_double("volume", v);
            if self.muted && v > 0. {
                let _ = m.set_flag("mute", false);
            }
        }
        self.volume = v;
    }

    /// O mpv é a fonte do volume; o controle só reflete (menos durante o arraste, que já mandou o valor).
    fn sync_volume_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = if self.muted { 0. } else { self.volume } as f32;
        if (self.volume_slider.read(cx).value().start() - shown).abs() >= 0.5 {
            self.volume_slider.update(cx, |s, cx| s.set_value(shown, window, cx));
        }
    }

    fn toggle_mute(&mut self) {
        if let Some(m) = &self.mpv {
            let _ = m.set_flag("mute", !self.muted);
        }
    }

    fn go_next(&mut self, cx: &mut Context<Self>) {
        let Some(n) = self.next() else { return };
        let from = self.file().name.clone();
        if self.advanced_from.as_deref() == Some(from.as_str()) {
            return;
        }
        self.advanced_from = Some(from);
        self.switch_to(n, cx);
    }

    fn switch_to(&mut self, file: FileEntry, cx: &mut Context<Self>) {
        self.record_progress(true);
        self.menu = None;
        self.target.file = file;
        self.load_current(None, cx);
    }

    /// O vídeo do arquivo atual já aparece na janela do mpv.
    fn video_ready(&self) -> bool {
        self.vo_configured && self.playing && self.broken.is_none()
    }

    fn show_skip(&self) -> bool {
        self.settings().show_skip_intro && self.skip.op.is_some_and(|op| self.pos >= op.start && self.pos < op.end - 1.)
    }

    fn skip_intro(&mut self) {
        if let (Some(op), true) = (self.skip.op, self.show_skip()) {
            self.seek(op.end);
        }
    }

    /// Início dos créditos: capítulos/AniSkip, senão estimativa (anime de TV), senão fim do arquivo.
    fn show_next(&self) -> bool {
        let ending = self.skip.ed.or_else(|| estimate_ending(self.dur, self.target.is_anime));
        self.next().is_some()
            && self.dur > 0.
            && !self.next_cancelled
            && self.pos >= next_card_start(self.dur, ending, self.settings().countdown_sec)
    }

    /// Liga/desliga a contagem do cartão de próximo episódio.
    fn update_countdown(&mut self, cx: &mut Context<Self>) {
        let want = self.show_next() && self.settings().auto_next;
        if !want {
            self.countdown = None;
            self.countdown_task = None;
            return;
        }
        if self.countdown.is_some() {
            return;
        }
        self.countdown = Some(self.settings().countdown_sec as f64);
        self.countdown_task = Some(cx.spawn(async move |this, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let done = this.update(cx, |v, cx| {
                    let now = Instant::now();
                    let step = if v.paused { 0. } else { (now - last).as_secs_f64() };
                    last = now;
                    let left = (v.countdown.unwrap_or(0.) - step).max(0.);
                    v.countdown = Some(left);
                    cx.notify();
                    if left <= 0. {
                        v.go_next(cx);
                        true
                    } else {
                        false
                    }
                });
                if done.unwrap_or(true) {
                    break;
                }
            }
        }));
    }

    fn choose_sub(&mut self, t: Option<Track>) {
        self.set_track("sid", t.as_ref().map_or(SubPick::Off, |t| SubPick::Id(t.id)));
        let prev = self.data.tracks.get(&self.target.dir).cloned().unwrap_or_default();
        let pref = TrackPref { sub_off: t.is_none(), sub_lang: t.as_ref().and_then(|t| t.lang.clone()), sub_title: t.and_then(|t| t.title), ..prev };
        self.save_tracks(pref);
    }

    fn choose_audio(&mut self, t: Track) {
        self.set_track("aid", SubPick::Id(t.id));
        let prev = self.data.tracks.get(&self.target.dir).cloned().unwrap_or_default();
        let pref = TrackPref { audio_lang: t.lang, audio_title: t.title, ..prev };
        self.save_tracks(pref);
    }

    fn save_tracks(&mut self, pref: TrackPref) {
        self.data.tracks.insert(self.target.dir.clone(), pref.clone());
        self.core.player().save_tracks(self.target.dir.clone(), pref);
    }

    fn change_settings(&mut self, f: impl FnOnce(&mut Settings)) {
        let mut s = self.data.settings.clone();
        f(&mut s);
        if s.cache_mib != self.data.settings.cache_mib
            && let Some(m) = &self.mpv
        {
            let _ = m.set("demuxer-max-bytes", &format!("{}MiB", s.cache_mib));
        }
        self.data.settings = s.clone();
        self.core.player().save_settings(s);
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        cx.emit(PlayerEvent::Close);
    }

    /// Mostra os controles e esconde de novo com o mouse parado.
    fn poke(&mut self, cx: &mut Context<Self>) {
        if self.idle {
            self.idle = false;
            cx.notify();
        }
        self.hide_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HIDE_AFTER).await;
            let _ = this.update(cx, |v, cx| {
                v.idle = true;
                cx.notify();
            });
        }));
    }

    fn controls_hidden(&self) -> bool {
        self.idle && !self.paused && self.menu.is_none() && !self.buffering && self.drag.is_none()
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = &e.keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        match e.keystroke.key.as_str() {
            "space" => self.toggle_pause(),
            "left" => self.seek(self.pos - 5.),
            "right" => self.seek(self.pos + 5.),
            "up" => self.set_volume(self.volume + 5.),
            "down" => self.set_volume(self.volume - 5.),
            "m" => self.toggle_mute(),
            "f" => window.toggle_fullscreen(),
            "n" => self.go_next(cx),
            "s" => self.skip_intro(),
            "escape" => {
                if self.menu.is_some() {
                    self.menu = None;
                } else if window.is_fullscreen() {
                    window.toggle_fullscreen();
                } else {
                    self.close(window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        self.poke(cx);
        cx.notify();
    }

    // --- Arrastar barras ---

    fn fraction(bounds: Bounds<Pixels>, x: Pixels) -> f64 {
        let w = f32::from(bounds.size.width).max(1.);
        ((f32::from(x - bounds.origin.x)) / w).clamp(0., 1.) as f64
    }

    fn drag_to(&mut self, x: Pixels) {
        if let Some(Drag::Seek(_)) = self.drag {
            self.drag = Some(Drag::Seek(Self::fraction(self.seek_bounds.get(), x) * self.dur));
        }
    }

    fn end_drag(&mut self) {
        if let Some(Drag::Seek(t)) = self.drag.take() {
            self.seek(t);
        }
    }
}

impl Drop for PlayerView {
    fn drop(&mut self) {
        self.record_progress(true);
        // O mpv recebe `quit` ao ser descartado; a janela filha some já.
        self.mpv = None;
        self.surface.hide();
    }
}

// --- Desenho ---
//
// Tudo aqui fica sobre o vídeo: texto e trilhos usam as cores de sobre-mídia
// (`theme::on_media*`, `media_scrim`), os controles são os do kit.

fn scrim_down(alpha: f32) -> gpui_kit::Background {
    linear_gradient(180., linear_color_stop(theme::media_scrim(alpha), 0.), linear_color_stop(theme::media_scrim(0.), 1.))
}

fn scrim_up(alpha: f32) -> gpui_kit::Background {
    linear_gradient(0., linear_color_stop(theme::media_scrim(alpha), 0.), linear_color_stop(theme::media_scrim(0.), 1.))
}

/// Cartão sobre o vídeo (erros, aviso de rede, próximo episódio).
fn media_card(cx: &App) -> Div {
    v_flex()
        .gap_2()
        .px_5()
        .py_4()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .text_color(cx.theme().popover_foreground)
        .shadow_lg()
}

fn menu_heading(text: &'static str, cx: &App) -> Div {
    div().pb_1().text_xs().font_weight(FontWeight::MEDIUM).text_color(cx.theme().muted_foreground).child(text)
}

const LANGS: [(&str, &str); 5] = [("por", "Português"), ("eng", "Inglês"), ("jpn", "Japonês"), ("kor", "Coreano"), ("spa", "Espanhol")];
const COUNTDOWNS: [u8; 3] = [5, 10, 15];
const CACHES: [u32; 4] = [100, 150, 300, 500];

impl PlayerView {
    fn track_label(t: &Track) -> String {
        match (t.title.as_ref(), t.lang.as_ref()) {
            (Some(_), Some(lang)) => format!("{} ({})", t.name(), lang.to_uppercase()),
            _ => t.name(),
        }
    }

    fn render_tracks(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let subs: Vec<Track> = self.tracks.iter().filter(|t| t.kind == TrackKind::Sub).cloned().collect();
        let audios: Vec<Track> = self.tracks.iter().filter(|t| t.kind == TrackKind::Audio).cloned().collect();
        // Índice 0 = desligada.
        let sub_ix = subs.iter().position(|t| t.selected).map_or(0, |i| i + 1);
        let audio_ix = audios.iter().position(|t| t.selected);
        let sub_labels: Vec<String> = std::iter::once("Desligada".to_owned()).chain(subs.iter().map(Self::track_label)).collect();
        v_flex()
            .w_72()
            .gap_3()
            .child(
                v_flex().child(menu_heading("Legenda", cx)).child(
                    RadioGroup::vertical("subtitle-tracks").children(radios(sub_labels)).selected_index(Some(sub_ix)).on_change(cx.listener(move |v, ix: &usize, _, cx| {
                        v.choose_sub(if *ix == 0 { None } else { subs.get(ix - 1).cloned() });
                        cx.notify();
                    })),
                ),
            )
            .when(audios.len() > 1, |p| {
                let labels: Vec<String> = audios.iter().map(Self::track_label).collect();
                p.child(Divider::horizontal()).child(
                    v_flex().child(menu_heading("Áudio", cx)).child(
                        RadioGroup::vertical("audio-tracks").children(radios(labels)).selected_index(audio_ix).on_change(cx.listener(move |v, ix: &usize, _, cx| {
                            if let Some(t) = audios.get(*ix) {
                                v.choose_audio(t.clone());
                                cx.notify();
                            }
                        })),
                    ),
                )
            })
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.settings().clone();
        let lang_group = |id: &'static str, label: &'static str, value: &str, sub: bool, cx: &mut Context<Self>| {
            v_flex().gap_1().child(menu_heading(label, cx)).child(
                RadioGroup::horizontal(id)
                    .children(radios(LANGS.iter().map(|(_, name)| *name)))
                    .selected_index(LANGS.iter().position(|(code, _)| *code == value))
                    .on_change(cx.listener(move |v, ix: &usize, _, cx| {
                        let code = LANGS[*ix].0;
                        v.change_settings(|s| if sub { s.sub_lang = code.into() } else { s.audio_lang = code.into() });
                        cx.notify();
                    })),
            )
        };
        v_flex()
            .w_80()
            .gap_3()
            .child(Switch::new("auto-next").cursor_pointer().label("Ir para o próximo episódio sozinho").checked(s.auto_next).on_change(cx.listener(|v, on: &bool, _, cx| {
                v.change_settings(|s| s.auto_next = *on);
                cx.notify();
            })))
            .when(s.auto_next, |p| {
                p.child(
                    v_flex().gap_1().child(menu_heading("Tempo da contagem", cx)).child(
                        RadioGroup::horizontal("countdown")
                            .children(radios(COUNTDOWNS.map(|n| format!("{n} s"))))
                            .selected_index(COUNTDOWNS.iter().position(|n| *n == s.countdown_sec))
                            .on_change(cx.listener(|v, ix: &usize, _, cx| {
                                v.change_settings(|s| s.countdown_sec = COUNTDOWNS[*ix]);
                                cx.notify();
                            })),
                    ),
                )
            })
            .child(Switch::new("skip-intro").cursor_pointer().label("Mostrar “Pular abertura”").checked(s.show_skip_intro).on_change(cx.listener(|v, on: &bool, _, cx| {
                v.change_settings(|s| s.show_skip_intro = *on);
                cx.notify();
            })))
            .child(Divider::horizontal())
            .child(lang_group("sub-lang", "Legenda preferida", &s.sub_lang, true, cx))
            .child(lang_group("audio-lang", "Áudio preferido", &s.audio_lang, false, cx))
            .child(Divider::horizontal())
            .child(
                v_flex().gap_1().child(menu_heading("Memória à frente", cx)).child(
                    RadioGroup::horizontal("cache")
                        .children(radios(CACHES.map(|n| format!("{n} MB"))))
                        .selected_index(CACHES.iter().position(|n| *n == s.cache_mib))
                        .on_change(cx.listener(|v, ix: &usize, _, cx| {
                            v.change_settings(|s| s.cache_mib = CACHES[*ix]);
                            cx.notify();
                        })),
                ),
            )
    }

    fn render_episodes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let current = self.file().name.clone();
        v_flex().id("episodes-list").w_96().max_h_96().overflow_y_scroll().child(menu_heading("Episódios", cx)).children(video_files(&self.target.files).into_iter().map(|f| {
            let n = episode_number(&f.name);
            let p = self.data.progress.get(&join_path(&self.target.dir, &f.name)).cloned();
            let is_current = f.name == current;
            let label = n.map_or_else(|| parse_episode(&f.name).label, |n| format!("Episódio {n}"));
            h_flex()
                .id(ElementId::Name(format!("ep:{}", f.name).into()))
                .gap_3()
                .px_2()
                .py_2()
                .rounded(theme.radius)
                .relative()
                .cursor_pointer()
                .map(|d| if is_current { d.bg(theme.list_active) } else { d.child(hover_layer("hover").fill(theme.list_hover).radius(theme.radius)) })
                .child(
                    div()
                        .w_8()
                        .flex_none()
                        .text_center()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_current { theme.primary_ink() } else { theme.muted_foreground })
                        .child(n.map_or("·".to_string(), |n| n.to_string())),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(truncate(label).text_sm())
                        .children(p.as_ref().filter(|p| !p.watched).map(|p| {
                            ProgressBar::new(ElementId::Name(format!("ep-progress:{}", f.name).into())).xsmall().value((p.position / p.duration.max(1.) * 100.).min(100.) as f32)
                        })),
                )
                .when(p.as_ref().is_some_and(|p| p.watched), |d| d.child(Icon::new(IconName::Check).small().text_color(theme.success_foreground)))
                .on_click(cx.listener(move |v, _, _, cx| v.switch_to(f.clone(), cx)))
        }))
    }

    /// Barra de posição própria: mostra o que já está na memória e os capítulos,
    /// coisa que o `Slider` do kit não tem.
    fn render_seekbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dur = self.dur.max(0.001);
        let shown = match self.drag {
            Some(Drag::Seek(t)) => t,
            _ => self.pos,
        };
        let frac = |t: f64| relative((t / dur).clamp(0., 1.) as f32);
        let bounds = self.seek_bounds.clone();
        let hover = self.hover;
        div()
            .id("seekbar")
            .relative()
            .flex()
            .items_center()
            .h_5()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, e: &MouseDownEvent, _, cx| {
                    if v.dur > 0. {
                        v.drag = Some(Drag::Seek(Self::fraction(v.seek_bounds.get(), e.position.x) * v.dur));
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                let b = v.seek_bounds.get();
                let t = Self::fraction(b, e.position.x) * v.dur;
                v.hover = Some((e.position.x - b.origin.x, t));
                v.want_thumb(t, cx);
                cx.notify();
            }))
            .on_hover(cx.listener(|v, h: &bool, _, cx| {
                if !h {
                    v.hover = None;
                    cx.notify();
                }
            }))
            .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().size_full())
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_1()
                    .rounded_full()
                    .bg(theme::media_track(0.2))
                    .child(div().absolute().top_0().bottom_0().left_0().rounded_full().bg(theme::media_track(0.35)).w(frac(self.pos + self.cache_ahead)))
                    .child(div().absolute().top_0().bottom_0().left_0().rounded_full().bg(theme.primary).w(frac(shown)))
                    // Divisas de capítulo: um traço escuro de 1 px físico sobre o trilho.
                    .children(self.chapters.iter().filter(|c| c.time > 0.).map(|c| div().absolute().top_0().bottom_0().w(px(2.)).bg(theme::media_scrim(0.8)).left(frac(c.time)))),
            )
            .when(self.dur > 0., |d| d.child(div().absolute().size_3p5().ml(rems(-0.4375)).rounded_full().bg(theme::on_media()).shadow_md().left(frac(shown))))
            .children(hover.map(|(x, t)| {
                let time = div().text_xs().font_weight(FontWeight::MEDIUM).child(format_time(t));
                let Some(th) = &self.thumbs else {
                    return div().absolute().bottom_6().left(x - px(24.)).px_2().py_0p5().rounded(theme.radius).bg(theme.popover).child(time);
                };
                // Prévia do quadro: centrada no mouse, sem passar das pontas da barra.
                let bar_w = self.seek_bounds.get().size.width;
                let left = (x - THUMB_W / 2.).min(bar_w - THUMB_W).max(px(0.));
                let frame = match th.get(thumbs::bucket(t)) {
                    Some(path) => img(path).w(THUMB_W).h(THUMB_H).object_fit(ObjectFit::Cover).rounded(theme.radius).into_any_element(),
                    None => div().w(THUMB_W).h(THUMB_H).rounded(theme.radius).bg(theme::media_scrim(0.6)).into_any_element(),
                };
                v_flex()
                    .absolute()
                    .bottom_6()
                    .left(left)
                    .items_center()
                    .gap_1()
                    .p_1()
                    .rounded(theme.radius_lg)
                    .bg(theme.popover)
                    .shadow_md()
                    .child(frame)
                    .child(time.pb_0p5())
            }))
    }

    fn menu_popover(&self, id: &'static str, menu: Menu, icon: IconName, tip: &'static str, content: AnyElement, cx: &mut Context<Self>) -> impl IntoElement {
        Popover::new(id)
            .anchor(gpui_kit::Anchor::BottomRight)
            .open(self.menu == Some(menu))
            .on_open_change(cx.listener(move |v, open: &bool, _, cx| {
                v.menu = if *open { Some(menu) } else if v.menu == Some(menu) { None } else { v.menu };
                cx.notify();
            }))
            .trigger(Button::new(ElementId::Name(format!("{id}-trigger").into())).ghost().icon(icon).tooltip(tip))
            .child(content)
    }

    fn render_controls(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vol_icon = if self.muted || self.volume == 0. {
            IconName::VolumeX
        } else if self.volume < 50. {
            IconName::Volume1
        } else {
            IconName::Volume2
        };
        let tracks = self.render_tracks(cx).into_any_element();
        let settings = self.render_settings(cx).into_any_element();
        let episodes = self.render_episodes(cx).into_any_element();
        h_flex()
            .gap_1()
            .child(Button::new("play").ghost().icon(if self.paused { IconName::Play } else { IconName::Pause }).tooltip(if self.paused { "Tocar (Espaço)" } else { "Pausar (Espaço)" }).on_click(cx.listener(|v, _, _, cx| {
                v.toggle_pause();
                cx.notify();
            })))
            .child(Button::new("next").ghost().icon(IconName::SkipForward).tooltip("Próximo episódio (N)").disabled(self.next().is_none()).on_click(cx.listener(|v, _, _, cx| v.go_next(cx))))
            .child(Button::new("mute").ghost().icon(vol_icon).tooltip(if self.muted { "Tirar do mudo (M)" } else { "Mudo (M)" }).on_click(cx.listener(|v, _, _, cx| {
                v.toggle_mute();
                cx.notify();
            })))
            .child(div().w_24().child(Slider::new(&self.volume_slider).accessibility_label("Volume")))
            .child(div().ml_2().flex_none().text_sm().text_color(theme::on_media_muted()).child(format!("{} / {}", format_time(self.pos), format_time(self.dur))))
            .child(h_flex().flex_1().min_w_0().px_4().justify_center().text_sm().font_weight(FontWeight::MEDIUM).child(truncate(episode_label(&self.file().name))))
            .child(self.menu_popover("tracks-menu", Menu::Tracks, IconName::Captions, "Legenda e áudio", tracks, cx))
            .child(self.menu_popover("settings-menu", Menu::Settings, IconName::Settings2, "Ajustes", settings, cx))
            .child(self.menu_popover("episodes-menu", Menu::Episodes, IconName::ListVideo, "Episódios", episodes, cx))
            .child(
                Button::new("fullscreen")
                    .ghost()
                    .icon(if window.is_fullscreen() { IconName::Minimize } else { IconName::Maximize })
                    .tooltip(if window.is_fullscreen() { "Sair da tela cheia (F)" } else { "Tela cheia (F)" })
                    .on_click(|_, window, _| window.toggle_fullscreen()),
            )
    }

    fn render_next_card(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.next().map_or_else(String::new, |n| episode_number(&n.name).map_or_else(|| n.name.clone(), |num| format!("Episódio {num}")));
        let total = self.settings().countdown_sec as f64;
        let fill = self.countdown.map_or(0., |left| 1. - left / total.max(1.));
        media_card(cx)
            .id("next-card")
            .occlude()
            .w_80()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(match self.countdown {
                Some(left) => format!("Próximo episódio em {} s", left.ceil() as u32),
                None => "Próximo episódio".into(),
            }))
            .child(truncate(label).font_weight(FontWeight::SEMIBOLD))
            .when(self.countdown.is_some(), |d| d.child(ProgressBar::new("next-countdown").xsmall().value((fill * 100.) as f32)))
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("watch-next").primary().icon(IconName::Play).label("Assistir agora").flex_1().on_click(cx.listener(|v, _, _, cx| v.go_next(cx))))
                    .child(Button::new("cancel-next").ghost().label("Cancelar").on_click(cx.listener(|v, _, _, cx| {
                        v.next_cancelled = true;
                        v.countdown = None;
                        v.countdown_task = None;
                        cx.notify();
                    }))),
            )
    }
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hidden = self.controls_hidden();
        // Estado da plataforma espelhando o da tela; idempotente.
        self.surface.set_cursor_hidden(hidden);
        let surface = self.surface.clone();
        let title = self.target.anime_title.clone();
        let ep = episode_label(&self.file().name);
        let show_next = self.show_next();
        let show_skip = self.show_skip() && !show_next;
        let theme = cx.theme().clone();
        let ready = self.video_ready();

        div()
            .id("player")
            .track_focus(&self.focus)
            .key_context("Player")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .text_color(theme::on_media())
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                if v.drag.is_some() {
                    v.drag_to(e.position.x);
                    cx.notify();
                }
                if v.last_mouse != Some(e.position) {
                    v.last_mouse = Some(e.position);
                    v.poke(cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|v, _: &MouseUpEvent, _, cx| {
                    v.end_drag();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|v, _: &MouseUpEvent, _, cx| {
                    v.end_drag();
                    cx.notify();
                }),
            )
            // Janela do mpv acompanha a área do player (pixels físicos).
            .child(
                canvas(
                    move |b, window, _| {
                        let s = window.scale_factor();
                        surface.place(
                            (f32::from(b.origin.x) * s).round() as i32,
                            (f32::from(b.origin.y) * s).round() as i32,
                            (f32::from(b.size.width) * s).round() as i32,
                            (f32::from(b.size.height) * s).round() as i32,
                        );
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            // Fundo até o primeiro quadro: sem ele a janela fica transparente
            // (o GPUI não pinta o fundo com o player aberto). Some com fade.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(theme.background)
                    .with_animation(SharedString::from(format!("backdrop:{ready}")), crate::motion::enter(crate::motion::BASE), move |e, t| {
                        e.opacity(if ready { 1. - t } else { 1. })
                    }),
            )
            // Topo sobreposto: some com o mouse parado (fade, sem mudar o layout).
            .child(
                    h_flex()
                        .flex_none()
                        .gap_2()
                        .h_12()
                        .pl_2()
                        .pr_4()
                        .bg(scrim_down(0.75))
                        .child(Button::new("back").ghost().icon(IconName::ArrowLeft).tooltip("Voltar (Esc)").on_click(cx.listener(|v, _, window, cx| v.close(window, cx))))
                        .child(
                            h_flex()
                                .min_w_0()
                                .flex_1()
                                .text_sm()
                                .child(truncate(title).flex_none().max_w(relative(0.7)).font_weight(FontWeight::SEMIBOLD))
                                .child(div().flex_none().text_color(theme::on_media_muted()).child(format!(" · {ep}"))),
                        )
                        .with_animation(SharedString::from(format!("top-bar:{hidden}")), crate::motion::enter(crate::motion::QUICK), move |e, t| {
                            e.opacity(if hidden { 1. - t } else { t })
                        }),
            )
            // Área do vídeo: clique pausa, duplo clique alterna tela cheia.
            .child(
                div()
                    .id("video-area")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .on_click(cx.listener(|v, e: &ClickEvent, window, cx| {
                        if v.menu.is_some() {
                            v.menu = None;
                        } else {
                            v.toggle_pause();
                            if e.click_count() == 2 {
                                window.toggle_fullscreen();
                            }
                        }
                        cx.notify();
                    }))
                    .when(self.buffering && !self.net_error && !self.fatal && self.broken.is_none(), |d| {
                        d.child(
                            v_flex()
                                .absolute()
                                .inset_0()
                                .justify_center()
                                .items_center()
                                .gap_3()
                                .child(Spinner::new().large().color(theme::on_media_muted()))
                                // Antes do primeiro quadro, diz o que está abrindo.
                                .when(!ready, |d| {
                                    d.child(div().text_sm().text_color(theme::on_media_muted()).child(format!("Abrindo {ep}…")))
                                }),
                        )
                    })
                    .when_some(self.broken.clone(), |d, msg| {
                        d.child(
                            h_flex().absolute().inset_0().justify_center().child(
                                media_card(cx)
                                    .items_center()
                                    .text_sm()
                                    .child(Icon::new(IconName::CircleAlert).text_color(theme.danger_foreground))
                                    .child("O motor de vídeo (mpv) não abriu.")
                                    .child(div().text_xs().text_color(theme.muted_foreground).child(msg)),
                            ),
                        )
                    })
                    .when(self.fatal, |d| {
                        d.child(
                            h_flex().absolute().inset_0().justify_center().child(
                                media_card(cx)
                                    .items_center()
                                    .text_sm()
                                    .child(Icon::new(IconName::CircleAlert).text_color(theme.danger_foreground))
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Não foi possível tocar este arquivo"))
                                    .child(div().max_w_72().text_center().text_xs().text_color(theme.muted_foreground).child("Pode ser a conexão, a sessão expirada ou um arquivo indisponível na nuvem."))
                                    .child(Button::new("retry").primary().small().icon(IconName::RefreshCw).label("Tentar de novo").on_click(cx.listener(|v, _, _, cx| {
                                        cx.stop_propagation();
                                        v.retry_attempt = 0;
                                        v.fatal = false;
                                        let at = v.pos;
                                        v.load_current(Some(at), cx);
                                    }))),
                            ),
                        )
                    })
                    .when(self.net_error, |d| {
                        d.child(
                            h_flex().absolute().inset_0().justify_center().child(
                                media_card(cx)
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .text_sm()
                                    .child(Icon::new(IconName::WifiOff).text_color(theme.danger_foreground))
                                    .child("Sem conexão com a nuvem. Tentando de novo…"),
                            ),
                        )
                    }),
            )
            // Pular abertura / próximo episódio: acima dos controles, mesmo com eles escondidos.
            .child(
                v_flex()
                    .absolute()
                    .right_5()
                    .map(|d| if hidden { d.bottom_6() } else { d.bottom_24() })
                    .items_end()
                    .gap_3()
                    .when(show_skip, |d| {
                        d.child(Button::new("skip-intro").icon(IconName::ChevronsRight).label("Pular abertura").tooltip("Pular abertura (S)").on_click(cx.listener(|v, _, _, cx| {
                            cx.stop_propagation();
                            v.skip_intro();
                            cx.notify();
                        })))
                    })
                    .when(show_next, |d| {
                        // O cartão aparece com fade: algo novo no canto.
                        d.child(self.render_next_card(cx).with_animation("next-card-in", crate::motion::enter(crate::motion::BASE), |e, t| {
                            e.opacity(t)
                        }))
                    }),
            )
            // Controles inferiores (mesmo fade do topo).
            .child(
                v_flex()
                    .id("controls")
                    .flex_none()
                    .gap_1()
                    .px_5()
                    .pt_10()
                    .pb_3()
                    .bg(scrim_up(0.85))
                    .child(self.render_seekbar(cx))
                    .child(self.render_controls(window, cx))
                    .with_animation(SharedString::from(format!("controls:{hidden}")), crate::motion::enter(crate::motion::QUICK), move |e, t| {
                        e.opacity(if hidden { 1. - t } else { t })
                    }),
            )
    }
}
