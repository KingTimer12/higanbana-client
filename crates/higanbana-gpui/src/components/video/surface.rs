//! `VideoSurface`: o quadro do vídeo ocupando o pai, com clique simples/duplo e cursor oculto.
use crate::components::Component;
use crate::cx::{EventCx, RenderCx, TimerId};
use crate::element::{div, img, Element, ElementId};
use crate::gfx::CursorStyle;
use crate::style::{ObjectFit, Styled};
use crate::video::Player;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Janela do duplo clique; o clique simples espera isso antes de disparar.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(250);

/// Clique simples aguardando um possível segundo clique. Fica na view.
#[derive(Debug, Default)]
pub struct ClickGate {
    pending: Option<(Instant, TimerId)>,
}

type Handler<V> = Rc<dyn Fn(&mut V, &mut EventCx)>;

/// `VideoSurface::new("sup", &mut v.player, |v| &mut v.gate).on_click(..).on_double_click(..)`.
pub struct VideoSurface<'s, V> {
    id: ElementId,
    player: &'s mut Player,
    gate: Rc<dyn Fn(&mut V) -> &mut ClickGate>,
    fit: ObjectFit,
    cursor_hidden: bool,
    on_click: Option<Handler<V>>,
    on_double: Option<Handler<V>>,
}

impl<'s, V: 'static> VideoSurface<'s, V> {
    pub fn new(id: impl Into<ElementId>, player: &'s mut Player, gate: impl Fn(&mut V) -> &mut ClickGate + 'static) -> Self {
        VideoSurface { id: id.into(), player, gate: Rc::new(gate), fit: ObjectFit::Contain, cursor_hidden: false, on_click: None, on_double: None }
    }

    pub fn fit(mut self, fit: ObjectFit) -> Self {
        self.fit = fit;
        self
    }

    /// Esconde o cursor sobre a superfície.
    pub fn cursor_hidden(mut self, hidden: bool) -> Self {
        self.cursor_hidden = hidden;
        self
    }

    /// Clique simples (depois de `DOUBLE_CLICK`, se não vier o segundo).
    pub fn on_click(mut self, f: impl Fn(&mut V, &mut EventCx) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// Duplo clique (cancela o simples).
    pub fn on_double_click(mut self, f: impl Fn(&mut V, &mut EventCx) + 'static) -> Self {
        self.on_double = Some(Rc::new(f));
        self
    }
}

fn clicked<V: 'static>(gate: &Rc<dyn Fn(&mut V) -> &mut ClickGate>, single: &Option<Handler<V>>, double: &Option<Handler<V>>, v: &mut V, cx: &mut EventCx) {
    if let Some((at, timer)) = gate(v).pending.take()
        && at.elapsed() <= DOUBLE_CLICK
    {
        cx.cancel_timer(timer);
        if let Some(f) = double {
            f(v, cx);
        }
        return;
    }
    let (g, f) = (gate.clone(), single.clone());
    let timer = cx.spawn_timer::<V>(DOUBLE_CLICK, move |v, cx| {
        g(v).pending = None;
        if let Some(f) = &f {
            f(v, cx);
        }
    });
    gate(v).pending = Some((Instant::now(), timer));
}

impl<V: 'static> Component<V> for VideoSurface<'_, V> {
    fn render(self, _cx: &mut RenderCx<'_, V>) -> Element<V> {
        let frame = self.player.frame().or_else(|| self.player.frame_source());
        let picture = frame.map(|f| img(f).absolute().inset(0.0).object_fit(self.fit));
        let (g, s, d) = (self.gate, self.on_click, self.on_double);
        let mut e = div().id(self.id).absolute().inset(0.0).items_center().justify_center().children(picture);
        if self.cursor_hidden {
            e = e.cursor(CursorStyle::None);
        }
        e.on_click(move |v: &mut V, _, cx| clicked(&g, &s, &d, v, cx))
    }
}
