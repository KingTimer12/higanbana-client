//! Tela do player: o quadro desenhado pela lib embaixo e a interface da master por cima.

mod controls;
mod corner;
mod keys;
mod menus;
mod seek;
mod status;
mod style;
mod top_bar;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::{Duration, Instant};

use higanbana_core::player::aniskip::SkipTimes;
use higanbana_core::Core;
use higanbana_gpui::components::{AutoHide, ClickGate, Component, PopoverState, SeekbarState, VideoSurface, HIDE_AFTER};
use higanbana_gpui::prelude::*;
use higanbana_gpui::ElementId;
use higanbana_gpui::video::{Player, PlayerState, Status};
use higanbana_gpui::{Point, Waker};

use super::options;
use super::session::{Action, PlayerSession};
use super::target::PlayTarget;
use super::thumbs::Thumbs;
use crate::image_cache::Decoded;
use crate::motion;
use crate::root::Higanbana;
use crate::state::{AppState, Post};

pub use keys::on_key;
pub use menus::Menu;

/// Janela do duplo clique na área do vídeo.
const DOUBLE_CLICK: Duration = Duration::from_millis(250);

/// Respostas de outras threads para o player (AniSkip, miniaturas), via `AppState`.
pub enum PlayerMsg {
    Skip(String, SkipTimes),
    Thumb(u64, u32, Option<Decoded>),
}

pub struct PlayerView {
    core: Arc<Core>,
    post: Post,
    pub player: Player,
    pub session: PlayerSession,
    /// Último estado lido do mpv (uma leitura por render).
    pub last: PlayerState,
    /// Falha ao iniciar o mpv (DLL ausente etc.).
    pub broken: Option<String>,
    pub menu: Option<Menu>,
    pub pops: [PopoverState; 3],
    pub hide: AutoHide,
    pub seek: SeekbarState,
    pub gate: ClickGate,
    thumbs: Option<Thumbs>,
    /// Arquivo das miniaturas (respostas de um arquivo anterior são ignoradas).
    load_seq: u64,
    last_click: Option<Instant>,
    last_mouse: Option<Point>,
}

/// O gerador de miniaturas pode ficar aberto? Tocando com os controles escondidos não
/// (o `sync` fecha e nada pode reabrir até os controles voltarem); pausado ou arrastando, sim.
pub fn thumbs_allowed(controls_hidden: bool, playing: bool, dragging: bool) -> bool {
    !(controls_hidden && playing && !dragging)
}

impl PlayerView {
    pub fn open(core: Arc<Core>, post: Post, target: PlayTarget, data: higanbana_core::player::store::PlayerData, waker: Waker) -> PlayerView {
        let now = Instant::now();
        let opts = options::mpv(&data.settings);
        let (player, broken) = match Player::with_options(waker, opts) {
            Ok(p) => (p, None),
            Err(e) => (Player::detached(), Some(e)),
        };
        let mut v = PlayerView {
            core,
            post,
            player,
            session: PlayerSession::new(target, data, now),
            last: PlayerState::default(),
            broken,
            menu: None,
            pops: Default::default(),
            hide: AutoHide::new(HIDE_AFTER, now),
            seek: SeekbarState::default(),
            gate: ClickGate::default(),
            thumbs: None,
            load_seq: 0,
            last_click: None,
            last_mouse: None,
        };
        if v.broken.is_none() {
            let a = v.session.start();
            v.run(vec![a]);
        }
        v
    }

    /// Executa o que a sessão pediu.
    pub fn run(&mut self, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::Load { path, size, start } => {
                    self.load_seq += 1;
                    self.thumbs = None;
                    self.menu = None;
                    self.pops = Default::default();
                    let url = self.core.player().stream_url(&path, size);
                    self.player.load_at(&url, start);
                }
                Action::SaveProgress { path, progress } => self.core.player().save_progress(path, progress),
                Action::SetTrack { prop, value } => self.player.set_property(prop, &value),
                Action::ResolveSkip { file, mal, ep, dur } => {
                    let core = self.core.clone();
                    let post = self.post.clone();
                    self.core.spawn(async move {
                        let times = core.player().aniskip(mal, ep, dur).await;
                        post(Box::new(move |s: &mut AppState, cx| {
                            s.player_inbox.push(PlayerMsg::Skip(file, times));
                            cx.notify();
                        }));
                    });
                }
            }
        }
    }

    /// O vídeo do arquivo atual já está na tela.
    pub fn video_ready(&self) -> bool {
        self.last.first_frame && matches!(self.last.status, Status::Playing | Status::Paused | Status::Ended) && self.broken.is_none()
    }

    /// Controles e cursor escondidos: parado, tocando, sem menu, sem cache, sem arraste.
    pub fn hidden(&self) -> bool {
        let hold = self.last.status != Status::Playing || self.menu.is_some() || self.last.buffering || self.seek.drag.is_some();
        self.hide.hidden(hold)
    }

    /// Movimento do mouse/tecla: mostra tudo de novo. As miniaturas só abrem no hover da
    /// barra (`want_thumb`): abrir a cada mexida punha um mpv e um pedido à nuvem por
    /// ciclo de mostrar/esconder os controles.
    pub fn poke(&mut self, now: Instant) {
        self.hide.poke(now);
    }

    /// Gerador de miniaturas do arquivo atual (só depois do primeiro quadro: não disputa a rede).
    fn ensure_thumbs(&mut self) {
        if !self.session.settings().thumbnails || !self.video_ready() || self.thumbs.is_some() {
            return;
        }
        // Mesma regra do `sync` que fecha: sem ela, um hover parado na barra reabria o
        // gerador a cada quadro (um mpv novo por vez, GBs de RAM e pedidos à nuvem).
        if !thumbs_allowed(self.hide.hidden(false), self.last.status == Status::Playing, self.seek.drag.is_some()) {
            return;
        }
        let seq = self.load_seq;
        let path = self.session.remote_path();
        let url = self.core.player().preview_url(&path, self.session.target.file.size);
        let dir = super::thumbs::thumbs_dir(seq);
        let post = self.post.clone();
        let mut th = Thumbs::open(url, dir, move |b, frame| {
            post(Box::new(move |s: &mut AppState, cx| {
                s.player_inbox.push(PlayerMsg::Thumb(seq, b, frame));
                cx.notify();
            }));
        });
        th.set_len(self.session.dur());
        self.thumbs = Some(th);
    }

    pub fn want_thumb(&mut self, t: f64) {
        self.ensure_thumbs();
        if let Some(th) = &mut self.thumbs {
            th.set_len(self.session.dur());
            th.want(t);
        }
    }

    pub fn thumb(&self, t: f64) -> Option<Option<ImageSource>> {
        self.thumbs.as_ref().map(|th| th.get(t))
    }

    fn on_msg(&mut self, m: PlayerMsg) {
        match m {
            PlayerMsg::Skip(file, times) => self.session.set_skip(&file, times),
            PlayerMsg::Thumb(seq, b, frame) => {
                if let Some(th) = self.thumbs.as_mut().filter(|_| seq == self.load_seq) {
                    th.finish(b, frame);
                }
            }
        }
    }

    /// Lê o mpv, aplica as regras e agenda o próximo redesenho.
    fn sync(&mut self, inbox: Vec<PlayerMsg>, cx: &mut RenderCx<'_, Higanbana>) {
        for m in inbox {
            self.on_msg(m);
        }
        let now = cx.now();
        self.last = self.player.state();
        let actions = self.session.on_state(&self.last, now);
        self.run(actions);
        if let Some(left) = self.hide.tick(now) {
            cx.request_redraw_in(left);
        } else if !thumbs_allowed(true, self.last.status == Status::Playing, self.seek.drag.is_some()) {
            // Controles sumiram: o segundo mpv (miniaturas) fecha e reabre no próximo movimento.
            self.thumbs = None;
        }
        if self.session.countdown().is_some() {
            cx.request_redraw_in(Duration::from_millis(100));
        }
        if let Some(at) = self.session.retry_at() {
            cx.request_redraw_in(at.saturating_duration_since(now));
        }
    }

    /// Clique na área do vídeo: fecha menu, senão pausa/toca; o segundo clique rápido
    /// também alterna a tela cheia (a pausa volta ao que era, como na master).
    pub fn click_video(&mut self, now: Instant) -> bool {
        if self.menu.take().is_some() {
            self.pops = Default::default();
            return false;
        }
        self.player.toggle_pause();
        let double = self.last_click.is_some_and(|t| now.saturating_duration_since(t) <= DOUBLE_CLICK);
        self.last_click = if double { None } else { Some(now) };
        double
    }

    /// Mouse mexeu (o mesmo ponto repetido não conta).
    pub fn mouse_moved(&mut self, p: Point, now: Instant) -> bool {
        if self.last_mouse == Some(p) {
            return false;
        }
        self.last_mouse = Some(p);
        self.poke(now);
        true
    }

    /// Fechando: grava onde parou.
    pub fn close(&mut self) {
        if let Some(a) = self.session.close(Instant::now()) {
            self.run(vec![a]);
        }
    }
}

/// Campos de reserva para handlers que chegam depois do player fechar (sem pânico).
#[derive(Default)]
pub struct Spare {
    pub seek: SeekbarState,
    pub gate: ClickGate,
    pub pop: PopoverState,
}

pub fn seek_of<'a>(p: &'a mut Option<PlayerView>, spare: &'a mut Spare) -> &'a mut SeekbarState {
    match p {
        Some(p) => &mut p.seek,
        None => &mut spare.seek,
    }
}

pub fn gate_of<'a>(p: &'a mut Option<PlayerView>, spare: &'a mut Spare) -> &'a mut ClickGate {
    match p {
        Some(p) => &mut p.gate,
        None => &mut spare.gate,
    }
}

pub fn pop_of<'a>(p: &'a mut Option<PlayerView>, spare: &'a mut Spare, i: usize) -> &'a mut PopoverState {
    match p {
        Some(p) => &mut p.pops[i],
        None => &mut spare.pop,
    }
}

pub fn view(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let inbox = std::mem::take(&mut v.state.player_inbox);
    let Some(p) = v.player.as_mut() else { return div() };
    p.sync(inbox, cx);
    let hidden = p.hidden();
    let ready = p.video_ready();
    let bg = cx.theme().background;
    let backdrop = cx.animate(ElementId::from("player-fundo"), if ready { 0.0 } else { 1.0 }, motion::transition(motion::BASE));
    let fade = cx.animate(ElementId::from("player-controles"), if hidden { 0.0 } else { 1.0 }, motion::transition(motion::QUICK));
    let surface = VideoSurface::new("player-video", &mut p.player, |v: &mut Higanbana| gate_of(&mut v.player, &mut v.player_spare)).cursor_hidden(hidden).render(cx);
    let top = top_bar::top_bar(v, cx, fade);
    let status = status::status(v, cx);
    let corner = corner::corner(v, cx, hidden);
    let controls = controls::controls(v, cx, fade);
    let area = div()
        .id("player-area")
        .relative()
        .flex_1()
        .min_h(0.0)
        .when(hidden, |d| d.cursor(CursorStyle::None))
        .on_click(|v: &mut Higanbana, _, cx| {
            if let Some(p) = v.player.as_mut()
                && p.click_video(Instant::now())
            {
                cx.window_command(WindowCommand::ToggleFullscreen);
            }
            cx.notify();
        })
        .children(status);
    div()
        .id("player")
        .track_focus("player")
        .relative()
        .size_full()
        .flex_col()
        .bg(Color::BLACK)
        .text_color(crate::theme::on_media())
        .on_key_down(on_key)
        .on_mouse_move_passive(|v: &mut Higanbana, ev, cx| {
            if let Some(p) = v.player.as_mut()
                && p.mouse_moved(ev.position, Instant::now())
            {
                cx.notify();
            }
        })
        .child(surface)
        .child(div().absolute().inset(0.0).bg(bg).opacity(backdrop))
        .child(top)
        .child(area)
        .child(corner)
        .child(controls)
}
