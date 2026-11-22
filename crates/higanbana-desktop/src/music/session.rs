//! Estado do player de músicas (a barra e o encarte leem daqui), guardado no `AppState`.
//! O motor manda um estado a cada ~250 ms; quando só o relógio andou, a tela do
//! álbum não refaz nada (listas em `Rc`) e só a barra muda.

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

use higanbana_core::music::album::{Album, Track};
use higanbana_core::music::engine::PlayerState;
use higanbana_core::music::queue::{QueueTrack, Repeat};
use higanbana_core::music::store::MusicPrefs;
use higanbana_core::music::TrackMeta;

use crate::state::AppState;

/// Salvar onde parou a cada tanto enquanto toca.
const SAVE_EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct MusicUi {
    /// Último estado do motor.
    pub player: Option<PlayerState>,
    pub album: Option<Rc<Album>>,
    pub metas: HashMap<String, TrackMeta>,
    queue: VecDeque<Track>,
    queued: HashSet<String>,
    running: bool,
    last_save: Option<Instant>,
    /// Fração escolhida enquanto o usuário arrasta a posição (o motor só pula ao soltar).
    pub seeking: Option<f32>,
    /// Volume antes de silenciar.
    pub unmuted: Option<f32>,
}

/// O que um estado novo do motor pede.
#[derive(Debug, PartialEq)]
pub struct StateOut {
    /// Algo na tela mudou.
    pub redraw: bool,
    /// Gravar onde parou: (álbum, faixa, segundos).
    pub save: Option<(String, usize, f64)>,
    pub prefs: Option<MusicPrefs>,
}

/// De `prev` para `next` só andaram posição e buffer.
pub fn only_clock(prev: PlayerState, next: &PlayerState) -> bool {
    PlayerState { position: next.position, buffered: next.buffered, ..prev } == *next
}

/// Próximo modo de repetição, com ícone e dica do atual.
pub fn repeat_cycle(r: Repeat) -> (&'static str, &'static str, Repeat) {
    match r {
        Repeat::Off => ("repeat", "Não repetir", Repeat::All),
        Repeat::All => ("repeat", "Repetir álbum", Repeat::One),
        Repeat::One => ("repeat-1", "Repetir faixa", Repeat::Off),
    }
}

impl MusicUi {
    pub fn on_state(&mut self, m: PlayerState, now: Instant) -> StateOut {
        let prev = self.player.replace(m.clone());
        let changed = prev.as_ref().is_none_or(|p| p.index != m.index || p.playing != m.playing || p.ended != m.ended);
        let due = self.last_save.is_none_or(|t| now.duration_since(t) >= SAVE_EVERY);
        let mut save = None;
        if (changed || (m.playing && due))
            && let (Some(album), Some(index)) = (self.album.as_ref(), m.index)
            && m.album.as_deref() == Some(album.path.as_str())
        {
            self.last_save = Some(now);
            // Álbum acabou: a próxima vez começa do início.
            let (i, at) = if m.ended { (0, 0.) } else { (index, m.position) };
            save = Some((album.path.clone(), i, at));
        }
        let prefs = prev
            .as_ref()
            .filter(|p| p.volume != m.volume || p.shuffle != m.shuffle || p.repeat != m.repeat || p.normalize != m.normalize)
            .map(|_| MusicPrefs { volume: m.volume, shuffle: m.shuffle, repeat: m.repeat, normalize: m.normalize });
        // Tique igual ao anterior (pausado): nada a redesenhar.
        let redraw = prev.as_ref() != Some(&m);
        StateOut { redraw, save, prefs }
    }

    pub fn is_current_album(&self, album: &Album) -> bool {
        self.player.as_ref().and_then(|m| m.album.as_deref()) == Some(album.path.as_str())
    }

    /// Há um álbum carregado no motor (a barra aparece).
    pub fn has_album(&self) -> bool {
        self.player.as_ref().is_some_and(|m| m.album.is_some())
    }
}

impl AppState {
    /// Estado novo do motor: grava progresso/preferências e diz se redesenha.
    pub fn music_state(&mut self, m: PlayerState) -> bool {
        let out = self.music.on_state(m, Instant::now());
        let store = &self.core.music().store;
        if let Some((path, i, at)) = &out.save {
            store.set_progress(path, *i, *at);
        }
        if let Some(p) = out.prefs {
            store.set_prefs(p);
        }
        out.redraw
    }

    pub fn play_album(&mut self, album: Rc<Album>, index: usize, at: f64) {
        let tracks = album.tracks.iter().map(|t| QueueTrack { path: t.path.clone(), size: t.size, ext: t.ext.clone() }).collect();
        self.core.music().play(album.path.clone(), tracks, index, at);
        self.music.album = Some(album);
    }

    /// Encarte aberto: retoma o álbum tocando e busca as tags das faixas, uma por vez.
    pub fn show_album(&mut self, album: &Rc<Album>) {
        let m = &mut self.music;
        if m.is_current_album(album) && m.album.as_ref().is_none_or(|a| a.path != album.path) {
            m.album = Some(album.clone());
        }
        for t in &album.tracks {
            if !m.metas.contains_key(&t.path) && m.queued.insert(t.path.clone()) {
                m.queue.push_back(t.clone());
            }
        }
        self.pump_meta();
    }

    fn pump_meta(&mut self) {
        if self.music.running {
            return;
        }
        let Some(t) = self.music.queue.pop_front() else { return };
        self.music.running = true;
        let core = self.core.clone();
        self.run(async move { (t.path.clone(), core.music().meta(&t.path, t.size, &t.ext, false).await) }, |s, (path, r), _| {
            s.music.running = false;
            if let Ok(m) = r {
                s.music.metas.insert(path, m);
            }
            s.pump_meta();
        });
    }

    pub fn toggle_mute(&mut self) {
        let Some(volume) = self.music.player.as_ref().map(|m| m.volume) else { return };
        let next = if volume <= 0.001 {
            self.music.unmuted.take().unwrap_or(0.8)
        } else {
            self.music.unmuted = Some(volume);
            0.
        };
        self.core.music().set(Some(next), None, None, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn album() -> Rc<Album> {
        Rc::new(Album { path: "a".into(), title: "A".into(), catalog: None, year: None, tracks: vec![], discs: vec![], cover_path: None, cover_size: None, others: vec![], disc_dirs: vec![] })
    }

    #[test]
    fn so_o_relogio() {
        let next = PlayerState { position: 1.25, buffered: 3., ..state() };
        assert!(only_clock(state(), &next));
    }

    #[test]
    fn troca_de_faixa_ou_pausa_nao_e_so_relogio() {
        let next = PlayerState { index: Some(1), position: 0., ..state() };
        assert!(!only_clock(state(), &next));
        let next = PlayerState { playing: false, ..state() };
        assert!(!only_clock(state(), &next));
    }

    #[test]
    fn salva_a_cada_5s() {
        let mut m = MusicUi { album: Some(album()), ..Default::default() };
        let t0 = Instant::now();
        assert!(m.on_state(state(), t0).save.is_some(), "primeiro estado grava");
        let s1 = PlayerState { position: 2., ..state() };
        let out = m.on_state(s1, t0 + Duration::from_secs(1));
        assert_eq!((out.save, out.redraw), (None, true), "relógio andou: redesenha, não grava");
        let s2 = PlayerState { position: 7., ..state() };
        assert_eq!(m.on_state(s2.clone(), t0 + Duration::from_secs(6)).save, Some(("a".into(), 0, 7.)));
        assert!(!m.on_state(s2, t0 + Duration::from_secs(7)).redraw, "mesmo estado: nada");
    }

    #[test]
    fn repetir_cicla() {
        let mut r = Repeat::Off;
        let mut seen = vec![];
        for _ in 0..3 {
            let (_, tip, next) = repeat_cycle(r);
            seen.push(tip);
            r = next;
        }
        assert_eq!(seen, ["Não repetir", "Repetir álbum", "Repetir faixa"]);
        assert_eq!(r, Repeat::Off);
    }
}
