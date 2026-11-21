//! Testes dos overlays sem GPU: render com relógio controlado + árvore + eventos.
mod dialog;
mod select;
mod tabs;
mod toast;
mod tooltip;

use crate::anim::Animator;
use crate::cx::{EventCx, FocusInfo, RenderCx, Timers};
use crate::gfx::{BackendKind, Stats};
use crate::layout::FakeMeasurer;
use crate::prelude::*;
use crate::{size, Modifiers, MouseButton, Point, TextStyle, Tree};
use std::time::{Duration, Instant};

/// Monta a view com tempo falso, mantendo animações e timers entre frames.
pub(crate) struct Harness {
    theme: Theme,
    anim: Animator,
    pub t0: Instant,
    pub now: Instant,
    pub timers: Timers,
    pub wake: Option<Instant>,
}

impl Harness {
    pub fn new() -> Self {
        let t0 = Instant::now();
        Self { theme: Theme::dark(), anim: Animator::default(), t0, now: t0, timers: Timers::default(), wake: None }
    }

    pub fn advance(&mut self, ms: u64) {
        self.now += Duration::from_millis(ms);
    }

    /// Renderiza com `f` e reconstrói `tree` (foco preservado).
    pub fn frame<V>(&mut self, tree: &mut Tree<V>, f: impl FnOnce(&mut RenderCx<'_, V>) -> Element<V>) {
        self.anim.begin();
        let mut cx = RenderCx::new(&self.theme, BackendKind::Auto, &[], Stats::default(), &mut self.anim)
            .with_focus(FocusInfo { focused: tree.focused(), visible: false });
        cx.now = self.now;
        let root = f(&mut cx);
        self.wake = cx.wake;
        self.anim.end();
        tree.rebuild(root, &TextStyle::default(), size(800.0, 600.0), &mut FakeMeasurer);
    }

    /// Recolhe pedidos de timer de um handler.
    pub fn absorb(&mut self, cx: &mut EventCx) {
        self.timers.absorb(cx, self.now);
    }
}

pub(crate) fn key(k: Key) -> KeyEvent {
    KeyEvent::new(k, Modifiers::default())
}

/// Clique completo (desce + sobe) em `p`.
pub(crate) fn click<V>(t: &mut Tree<V>, v: &mut V, p: Point) -> EventCx {
    let mut cx = EventCx::default();
    t.mouse_down(p, MouseButton::Left, Modifiers::default(), v, &mut cx);
    t.mouse_up(p, MouseButton::Left, Modifiers::default(), v, &mut cx);
    cx
}

/// Centro do elemento `id`.
pub(crate) fn center<V>(t: &Tree<V>, id: impl Into<crate::ElementId>) -> Point {
    let r = t.rect(t.find(id).expect("elemento na árvore"));
    crate::point(r.origin.x + r.size.width / 2.0, r.origin.y + r.size.height / 2.0)
}
