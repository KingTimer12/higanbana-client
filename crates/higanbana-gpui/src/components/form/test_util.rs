//! Apoio aos testes do grupo `form`: árvore sem GPU e canvas que grava chamadas.
use crate::anim::Animator;
use crate::layout::FakeMeasurer;
use crate::paint::Canvas;
use crate::prelude::*;
use crate::{size, Corners, Modifiers, Point, Quad, Rect, Stats, TextStyle, Tree};

pub(super) fn build<V>(root: Element<V>) -> Tree<V> {
    let mut t = Tree::new();
    t.rebuild(root, &TextStyle::default(), size(300.0, 200.0), &mut FakeMeasurer);
    t
}

pub(super) fn key(k: Key) -> KeyEvent {
    KeyEvent::new(k, Modifiers::default())
}

/// Monta um `RenderCx` de teste e chama `f` com ele.
pub(super) fn with_cx<V, R>(anim: &mut Animator, f: impl FnOnce(&mut RenderCx<'_, V>) -> R) -> R {
    let theme = Theme::dark();
    let mut cx = RenderCx::new(&theme, BackendKind::Auto, &[], Stats::default(), anim);
    f(&mut cx)
}

/// Textos e quads pintados.
#[derive(Default)]
pub(super) struct Rec {
    pub texts: Vec<String>,
    pub quads: Vec<(Rect, Color)>,
}

impl Canvas for Rec {
    fn quad(&mut self, q: &Quad) {
        self.quads.push((q.rect, q.fill));
    }
    fn image(&mut self, _: &ImageSource, _: Rect, _: Corners<f32>, _: Color) {}
    fn text(&mut self, t: &str, _: &TextStyle, _: Point, _: Option<f32>, _: Color) {
        self.texts.push(t.into());
    }
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {}
}

pub(super) fn paint<V>(t: &Tree<V>) -> Rec {
    let mut r = Rec::default();
    t.paint(&mut r);
    r
}
