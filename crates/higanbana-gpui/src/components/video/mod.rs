//! `VideoPlayer`: superfície de vídeo (libmpv) com controles que somem sozinhos,
//! atalhos de teclado e duplo clique para tela cheia. Requer `feature = "video"`.
mod controls;
mod player_view;
#[cfg(test)]
mod tests;

use crate::components::SelectState;
use crate::cx::TimerId;
use crate::element::ElementId;
use crate::style::ObjectFit;
use crate::video::Player;
use std::rc::Rc;
use std::time::Instant;

/// Estado do player guardado na view: o `Player` e o que a UI lembra entre frames.
pub struct VideoState {
    pub player: Player,
    last_activity: Instant,
    /// Clique simples aguardando um possível segundo clique (duplo = tela cheia).
    pending_click: Option<(Instant, TimerId)>,
    /// Controles visíveis no último render (o mouse só reconstrói se estavam ocultos).
    controls_visible: bool,
    /// Ainda não pegou o foco do teclado (pega no primeiro movimento do mouse).
    want_focus: bool,
    speed: SelectState,
    audio: SelectState,
    sub: SelectState,
}

impl VideoState {
    pub fn new(player: Player) -> Self {
        VideoState {
            player,
            last_activity: Instant::now(),
            pending_click: None,
            controls_visible: true,
            want_focus: true,
            speed: SelectState::default(),
            audio: SelectState::default(),
            sub: SelectState::default(),
        }
    }

    /// Algum menu da barra aberto: os controles não podem sumir.
    fn menu_open(&self) -> bool {
        self.speed.open || self.audio.open || self.sub.open
    }

    /// Mouse/teclado: mostra os controles e reinicia a contagem para sumirem.
    fn touch(&mut self) -> bool {
        self.last_activity = Instant::now();
        !self.controls_visible
    }
}

pub(crate) type Lens<V> = Rc<dyn Fn(&mut V) -> &mut VideoState>;

/// `VideoPlayer::new("player", &mut view.video, |v: &mut View| &mut v.video).render(cx)`.
pub struct VideoPlayer<'s, V> {
    id: ElementId,
    state: &'s mut VideoState,
    lens: Lens<V>,
    fit: ObjectFit,
}

impl<'s, V: 'static> VideoPlayer<'s, V> {
    pub fn new(id: impl Into<ElementId>, state: &'s mut VideoState, lens: impl Fn(&mut V) -> &mut VideoState + 'static) -> Self {
        VideoPlayer { id: id.into(), state, lens: Rc::new(lens), fit: ObjectFit::Contain }
    }

    /// `Contain` (padrão, com barras) ou `Cover` (preenche e corta).
    pub fn fit(mut self, fit: ObjectFit) -> Self {
        self.fit = fit;
        self
    }
}

/// Id filho estável derivado do id do player.
pub(crate) fn part(id: ElementId, name: &str) -> ElementId {
    ElementId(id.0.rotate_left(13) ^ ElementId::from(name).0)
}

pub(crate) fn surface_id(id: ElementId) -> ElementId {
    part(id, "superficie")
}

pub(crate) fn controls_id(id: ElementId) -> ElementId {
    part(id, "controles")
}

/// `m:ss`, ou `h:mm:ss` a partir de uma hora.
pub(crate) fn fmt_time(secs: f64) -> String {
    let s = if secs.is_finite() { secs.max(0.0) as u64 } else { 0 };
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}
