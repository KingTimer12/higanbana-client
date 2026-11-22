//! `Seekbar`: barra de progresso de mídia com buffer, capítulos, knob, arraste e conteúdo no hover.
use super::part;
use crate::color::{rgba, Color};
use crate::components::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, Element, ElementId};
use crate::style::Styled;
use std::rc::Rc;

const HIT: f32 = 20.0;
const TRACK: f32 = 4.0;
const KNOB: f32 = 14.0;
const MARK: f32 = 2.0;
/// Distância do conteúdo de hover acima da base da barra.
pub const SEEKBAR_HOVER_BOTTOM: f32 = 24.0;

/// Estado na view: fração em arraste e x local do hover.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SeekbarState {
    pub drag: Option<f64>,
    pub hover: Option<f32>,
    /// Largura da barra no último evento do mouse.
    pub width: f32,
}

type SeekFn<V> = Rc<dyn Fn(&mut V, f64, &mut EventCx)>;
type DragFn<V> = Rc<dyn Fn(&mut V, bool, &mut EventCx)>;
type HoverFn<V> = Box<dyn Fn(f64, f32, f32) -> Element<V>>;

/// `Seekbar::new("sb", &v.bar, |v| &mut v.bar).duration(d).position(p).on_seek(..)`.
pub struct Seekbar<'a, V> {
    id: ElementId,
    state: &'a SeekbarState,
    lens: Rc<dyn Fn(&mut V) -> &mut SeekbarState>,
    duration: f64,
    position: f64,
    buffered: f64,
    marks: Vec<f64>,
    played: Color,
    hover: Option<HoverFn<V>>,
    on_seek: Option<SeekFn<V>>,
    on_drag: Option<DragFn<V>>,
}

impl<'a, V: 'static> Seekbar<'a, V> {
    pub fn new(id: impl Into<ElementId>, state: &'a SeekbarState, lens: impl Fn(&mut V) -> &mut SeekbarState + 'static) -> Self {
        Seekbar {
            id: id.into(),
            state,
            lens: Rc::new(lens),
            duration: 0.0,
            position: 0.0,
            buffered: 0.0,
            marks: Vec::new(),
            played: rgba(0xffff_ffff),
            hover: None,
            on_seek: None,
            on_drag: None,
        }
    }

    pub fn duration(mut self, secs: f64) -> Self {
        self.duration = secs;
        self
    }

    pub fn position(mut self, secs: f64) -> Self {
        self.position = secs;
        self
    }

    /// Até onde já foi baixado (segundos absolutos).
    pub fn buffered(mut self, secs: f64) -> Self {
        self.buffered = secs;
        self
    }

    /// Capítulos (segundos); `0` é ignorado.
    pub fn marks(mut self, marks: Vec<f64>) -> Self {
        self.marks = marks;
        self
    }

    pub fn played_color(mut self, c: Color) -> Self {
        self.played = c;
        self
    }

    /// Conteúdo acima da barra no hover: `(segundos, x local, largura da barra)`.
    pub fn hover_content(mut self, f: impl Fn(f64, f32, f32) -> Element<V> + 'static) -> Self {
        self.hover = Some(Box::new(f));
        self
    }

    /// Soltou (clique ou fim do arraste) em `segundos`.
    pub fn on_seek(mut self, f: impl Fn(&mut V, f64, &mut EventCx) + 'static) -> Self {
        self.on_seek = Some(Rc::new(f));
        self
    }

    /// Começou (`true`) / terminou (`false`) o arraste.
    pub fn on_drag(mut self, f: impl Fn(&mut V, bool, &mut EventCx) + 'static) -> Self {
        self.on_drag = Some(Rc::new(f));
        self
    }

    pub fn knob_id(id: ElementId) -> ElementId {
        part(id, "knob")
    }

    pub fn buffer_id(id: ElementId) -> ElementId {
        part(id, "buffer")
    }

    pub fn mark_id(id: ElementId, i: usize) -> ElementId {
        ElementId(part(id, "marca").0.wrapping_add(i as u64))
    }
}

fn frac(x: f32, w: f32) -> f64 {
    if w <= 0.0 { 0.0 } else { (x / w).clamp(0.0, 1.0) as f64 }
}

fn pct(f: f64) -> crate::style::Length {
    crate::style::pct((f.clamp(0.0, 1.0) * 100.0) as f32)
}

impl<V: 'static> Component<V> for Seekbar<'_, V> {
    fn render(self, _cx: &mut RenderCx<'_, V>) -> Element<V> {
        let (id, dur) = (self.id, self.duration);
        let ratio = |s: f64| if dur > 0.0 { s / dur } else { 0.0 };
        let shown = self.state.drag.unwrap_or(ratio(self.position));
        let mut track = div()
            .relative()
            .w_full()
            .h(TRACK)
            .rounded_full()
            .bg(rgba(0xffff_ff33))
            .child(div().id(Self::buffer_id(id)).absolute().top(0.0).left(0.0).h_full().rounded_full().w(pct(ratio(self.buffered))).bg(rgba(0xffff_ff59)))
            .child(div().absolute().top(0.0).left(0.0).h_full().rounded_full().w(pct(shown)).bg(self.played));
        for (i, m) in self.marks.iter().enumerate() {
            if *m > 0.0 && dur > 0.0 {
                track = track.child(div().id(Self::mark_id(id, i)).absolute().top(0.0).h_full().w(MARK).left(pct(ratio(*m))).bg(Color::BLACK.alpha(0.8)));
            }
        }
        if dur > 0.0 {
            let knob = div().id(Self::knob_id(id)).size(KNOB).rounded_full().bg(rgba(0xffff_ffff)).shadow(Color::BLACK.alpha(0.1), 6.0);
            track = track.child(div().absolute().top(-(KNOB - TRACK) / 2.0).left(pct(shown)).ml(-KNOB / 2.0).child(knob));
        }
        let (l1, l2, l3, l4) = (self.lens.clone(), self.lens.clone(), self.lens.clone(), self.lens.clone());
        let (d1, d2, seek) = (self.on_drag.clone(), self.on_drag, self.on_seek);
        let mut root = div()
            .id(id)
            .relative()
            .w_full()
            .h(HIT)
            .items_center()
            .cursor_pointer()
            .on_mouse_down(move |v: &mut V, ev, cx| {
                l1(v).drag = Some(frac(ev.local.x, ev.bounds.size.width));
                if let Some(f) = &d1 {
                    f(v, true, cx);
                }
                cx.stop_propagation();
                cx.notify();
            })
            .on_mouse_move(move |v: &mut V, ev, cx| {
                let s = l2(v);
                s.width = ev.bounds.size.width;
                let inside = ev.local.y >= 0.0 && ev.local.y <= ev.bounds.size.height && ev.local.x >= 0.0 && ev.local.x <= ev.bounds.size.width;
                if s.drag.is_some() {
                    s.drag = Some(frac(ev.local.x, ev.bounds.size.width));
                }
                s.hover = (inside || s.drag.is_some()).then_some(ev.local.x.clamp(0.0, ev.bounds.size.width));
                cx.notify();
            })
            .on_mouse_up(move |v: &mut V, _, cx| {
                let Some(f) = l3(v).drag.take() else { return };
                if let Some(s) = &seek {
                    s(v, f * dur, cx);
                }
                if let Some(d) = &d2 {
                    d(v, false, cx);
                }
                cx.notify();
            })
            .on_hover(move |v: &mut V, on, cx| {
                let s = l4(v);
                if !on && s.drag.is_none() {
                    s.hover = None;
                    cx.notify();
                }
            })
            .child(track);
        if let (Some(f), Some(x)) = (&self.hover, self.state.hover) {
            let w = self.state.width;
            root = root.child(div().absolute().bottom(SEEKBAR_HOVER_BOTTOM).left(0.0).child(f(frac(x, w) * dur, x, w)));
        }
        root
    }
}
