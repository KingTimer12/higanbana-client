//! Contexto de `View::render`: tema, backends, foco e animações.
use crate::anim::{Animatable, Animator, Transition};
use crate::element::ElementId;
use crate::gfx::{BackendInfo, BackendKind, Stats, Waker, WindowState};
use crate::theme::Theme;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

/// Estado de foco visto pelo render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FocusInfo {
    pub focused: Option<ElementId>,
    /// O foco veio do teclado (Tab): mostrar o anel de foco.
    pub visible: bool,
}

/// Contexto de `View::render`.
pub struct RenderCx<'a, V: ?Sized> {
    pub(crate) theme: &'a Theme,
    pub(crate) backend: BackendKind,
    pub(crate) backends: &'a [BackendInfo],
    pub(crate) stats: Stats,
    pub(crate) focus: FocusInfo,
    pub(crate) now: Instant,
    pub(crate) animator: &'a mut Animator,
    /// Próximo instante em que a view pediu para ser reconstruída.
    pub(crate) wake: Option<Instant>,
    pub(crate) window: WindowState,
    pub(crate) waker: Waker,
    pub(crate) _view: PhantomData<fn(&mut V)>,
}

impl<'a, V: ?Sized> RenderCx<'a, V> {
    pub fn new(theme: &'a Theme, backend: BackendKind, backends: &'a [BackendInfo], stats: Stats, animator: &'a mut Animator) -> Self {
        Self { theme, backend, backends, stats, focus: FocusInfo::default(), now: Instant::now(), animator, wake: None, window: WindowState::default(), waker: Waker::noop(), _view: PhantomData }
    }

    pub(crate) fn with_focus(mut self, focus: FocusInfo) -> Self {
        self.focus = focus;
        self
    }

    /// Acorda o app de outra thread (ex.: `video::Player` com quadro novo).
    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    /// Maximizada, fullscreen, com foco...
    pub fn window_state(&self) -> WindowState {
        self.window
    }

    pub fn theme(&self) -> &'a Theme {
        self.theme
    }

    /// Backend ativo.
    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    /// Backends disponíveis nesta máquina.
    pub fn backends(&self) -> &'a [BackendInfo] {
        self.backends
    }

    /// Estatísticas do último frame.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Instante deste render (o mesmo para todas as animações do frame).
    pub fn now(&self) -> Instant {
        self.now
    }

    /// O elemento `id` tem o foco?
    pub fn is_focused(&self, id: impl Into<ElementId>) -> bool {
        self.focus.focused == Some(id.into())
    }

    /// Foco atual e se o anel de foco deve aparecer (navegação por teclado).
    pub fn focus(&self) -> FocusInfo {
        self.focus
    }

    /// Valor animado de `key` rumo a `target`; a primeira consulta já devolve `target`.
    /// Enquanto anima, o app reconstrói a cada frame; depois volta a dormir.
    pub fn animate(&mut self, key: impl Into<ElementId>, target: f32, transition: Transition) -> f32 {
        self.animator.value(key.into(), target, target, transition, self.now)
    }

    /// Como `animate`, mas a primeira consulta parte de `from` (animação de entrada).
    pub fn animate_from(&mut self, key: impl Into<ElementId>, from: f32, target: f32, transition: Transition) -> f32 {
        self.animator.value(key.into(), from, target, transition, self.now)
    }

    /// `animate` para cor, ponto, tamanho ou retângulo.
    pub fn animate_value<T: Animatable>(&mut self, key: impl Into<ElementId>, target: T, transition: Transition) -> T {
        self.animator.animate(key.into(), target, target, transition, self.now)
    }

    /// `animate_from` para qualquer `Animatable`.
    pub fn animate_value_from<T: Animatable>(&mut self, key: impl Into<ElementId>, from: T, target: T, transition: Transition) -> T {
        self.animator.animate(key.into(), from, target, transition, self.now)
    }

    /// Presença 0..1 de algo que aparece/some (entra de 0, sai animando até 0).
    /// `None` = não renderizar (invisível e saída concluída).
    pub fn presence(&mut self, key: impl Into<ElementId>, visible: bool, transition: Transition) -> Option<f32> {
        self.animator.presence(key.into(), visible, transition, self.now)
    }

    /// Progresso 0..1 que se repete para sempre (spinners, pulsos). Força `Repeat::Forever`;
    /// para de gerar frames assim que o render deixa de consultar `key`.
    pub fn animate_loop(&mut self, key: impl Into<ElementId>, transition: Transition) -> f32 {
        let t = transition.repeat(crate::anim::Repeat::Forever);
        self.animator.value(key.into(), 0.0, 1.0, t, self.now)
    }

    /// Reconstrói de novo depois de `delay` (ex.: relógio, piscar algo).
    pub fn request_redraw_in(&mut self, delay: Duration) {
        let at = self.now + delay;
        self.wake = Some(self.wake.map_or(at, |w| w.min(at)));
    }

    /// Reconstrói no próximo frame.
    pub fn request_animation_frame(&mut self) {
        self.request_redraw_in(Duration::ZERO);
    }
}
