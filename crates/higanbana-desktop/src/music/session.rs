//! Estado do player de músicas (a barra e o encarte leem daqui).
//!
//! Entidade própria, fora do `AppState`: o motor manda um estado a cada ~250 ms e
//! só a barra precisa da posição. Quem depende só de faixa/play/tags escuta
//! `MusicEvent::Changed` em vez de redesenhar a cada tique.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::{Context, EventEmitter};
use higanbana_core::{
    Core,
    music::{
        TrackMeta,
        album::{Album, Track},
        engine::PlayerState,
        queue::QueueTrack,
        store::MusicPrefs,
    },
};

/// Salvar onde parou a cada tanto enquanto toca.
const SAVE_EVERY: Duration = Duration::from_secs(5);

/// Mudou algo além do relógio (posição/buffer): faixa, play/pausa, tags, álbum.
pub enum MusicEvent {
    Changed,
}

pub struct MusicSession {
    core: Arc<Core>,
    /// Último estado do motor.
    pub player: Option<PlayerState>,
    pub album: Option<Album>,
    pub metas: HashMap<String, TrackMeta>,
    queue: VecDeque<Track>,
    queued: HashSet<String>,
    running: bool,
    last_save: Option<Instant>,
}

impl EventEmitter<MusicEvent> for MusicSession {}

impl MusicSession {
    pub fn new(core: Arc<Core>) -> Self {
        Self {
            core,
            player: None,
            album: None,
            metas: HashMap::new(),
            queue: VecDeque::new(),
            queued: HashSet::new(),
            running: false,
            last_save: None,
        }
    }

    pub fn open(&mut self, album: Album, index: usize, at: f64, cx: &mut Context<Self>) {
        let tracks = album.tracks.iter().map(|t| QueueTrack { path: t.path.clone(), size: t.size, ext: t.ext.clone() }).collect();
        self.core.music().play(album.path.clone(), tracks, index, at);
        self.album = Some(album);
        cx.emit(MusicEvent::Changed);
        cx.notify();
    }

    pub fn command(&self, cmd: &str) {
        let _ = self.core.music().command(cmd);
    }

    pub fn is_current_album(&self, album: &Album) -> bool {
        self.player.as_ref().and_then(|m| m.album.as_deref()) == Some(album.path.as_str())
    }

    /// Encarte aberto: retoma o álbum tocando e busca as tags das faixas, uma por vez.
    pub fn show(&mut self, album: &Album, cx: &mut Context<Self>) {
        if self.is_current_album(album) && self.album.as_ref().is_none_or(|a| a.path != album.path) {
            self.album = Some(album.clone());
        }
        for t in &album.tracks {
            if !self.metas.contains_key(&t.path) && self.queued.insert(t.path.clone()) {
                self.queue.push_back(t.clone());
            }
        }
        self.pump_meta(cx);
    }

    fn pump_meta(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        let Some(t) = self.queue.pop_front() else { return };
        self.running = true;
        let core = self.core.clone();
        let task = self.core.spawn(async move { (t.path.clone(), core.music().meta(&t.path, t.size, &t.ext, false).await) });
        cx.spawn(async move |this, cx| {
            let r = task.await.ok();
            let _ = this.update(cx, |s, cx| {
                s.running = false;
                if let Some((path, Ok(m))) = r {
                    s.metas.insert(path, m);
                    cx.emit(MusicEvent::Changed);
                    cx.notify();
                }
                s.pump_meta(cx);
            });
        })
        .detach();
    }

    pub fn on_state(&mut self, m: &PlayerState, cx: &mut Context<Self>) {
        let prev = self.player.replace(m.clone());
        let changed = prev.as_ref().is_none_or(|p| p.index != m.index || p.playing != m.playing || p.ended != m.ended);
        let due = self.last_save.is_none_or(|t| t.elapsed() >= SAVE_EVERY);
        if (changed || (m.playing && due))
            && let (Some(album), Some(index)) = (self.album.as_ref(), m.index)
            && m.album.as_deref() == Some(album.path.as_str())
        {
            self.last_save = Some(Instant::now());
            // Álbum acabou: a próxima vez começa do início.
            let (i, at) = if m.ended { (0, 0.) } else { (index, m.position) };
            self.core.music().store.set_progress(&album.path, i, at);
        }
        if let Some(p) = &prev
            && (p.volume != m.volume || p.shuffle != m.shuffle || p.repeat != m.repeat || p.normalize != m.normalize)
        {
            self.core.music().store.set_prefs(MusicPrefs { volume: m.volume, shuffle: m.shuffle, repeat: m.repeat, normalize: m.normalize });
        }
        // Tique igual ao anterior (pausado): nada a redesenhar.
        if prev.as_ref() == Some(m) {
            return;
        }
        // Só o relógio andou: basta a barra (que observa a entidade).
        if !prev.is_some_and(|p| only_clock(p, m)) {
            cx.emit(MusicEvent::Changed);
        }
        cx.notify();
    }
}

/// De `prev` para `next` só andaram posição e buffer.
fn only_clock(prev: PlayerState, next: &PlayerState) -> bool {
    PlayerState { position: next.position, buffered: next.buffered, ..prev } == *next
}

#[cfg(test)]
mod tests {
    use super::*;
    use higanbana_core::music::queue::Repeat;

    fn state() -> PlayerState {
        PlayerState {
            album: Some("a".into()),
            index: Some(0),
            path: Some("a/1.flac".into()),
            playing: true,
            position: 1.,
            duration: Some(100.),
            buffered: 2.,
            format: None,
            volume: 0.8,
            shuffle: false,
            repeat: Repeat::Off,
            normalize: false,
            error: None,
            loading: false,
            ended: false,
            title: None,
            artist: None,
        }
    }

    #[test]
    fn so_o_relogio() {
        let prev = state();
        let next = PlayerState { position: 1.25, buffered: 3., ..state() };
        assert!(only_clock(prev, &next));
    }

    #[test]
    fn troca_de_faixa_ou_pausa_nao_e_so_relogio() {
        let next = PlayerState { index: Some(1), position: 0., ..state() };
        assert!(!only_clock(state(), &next));
        let next = PlayerState { playing: false, ..state() };
        assert!(!only_clock(state(), &next));
    }
}
