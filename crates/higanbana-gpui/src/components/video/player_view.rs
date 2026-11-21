//! Render do `VideoPlayer`: superfície, estados (carregando/erro), teclado e cliques.
use super::{controls, controls_id, surface_id, Lens, VideoPlayer};
use crate::anim::Transition;
use crate::color::rgba;
use crate::components::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, img, text, Element};
use crate::gfx::WindowCommand;
use crate::input::{Key, KeyEvent};
use crate::style::Styled;
use crate::video::Status;
use std::time::{Duration, Instant};

/// Tempo sem mexer até os controles sumirem (tocando).
const HIDE_AFTER: Duration = Duration::from_millis(2500);
/// Janela do duplo clique; o clique simples espera isso antes de pausar.
const DOUBLE_CLICK: Duration = Duration::from_millis(250);
const FADE: Transition = Transition::ms(200);

fn on_key<V: 'static>(lens: &Lens<V>, v: &mut V, ev: &KeyEvent, cx: &mut EventCx) {
    let s = lens(v);
    let p = &s.player;
    let vol = p.state().volume;
    match ev.key {
        Key::Space | Key::Char('k') => p.toggle_pause(),
        Key::Left => p.seek_relative(-5.0),
        Key::Right => p.seek_relative(5.0),
        Key::Up => p.set_volume(vol + 5.0),
        Key::Down => p.set_volume(vol - 5.0),
        Key::Char('m') => p.toggle_mute(),
        Key::Char('f') => cx.window_command(WindowCommand::ToggleFullscreen),
        Key::Escape => cx.window_command(WindowCommand::ExitFullscreen),
        _ => return,
    }
    s.touch();
    cx.notify();
    cx.stop_propagation();
}

/// Clique simples pausa depois de `DOUBLE_CLICK`; o segundo clique cancela e alterna tela cheia.
fn on_surface_click<V: 'static>(lens: &Lens<V>, v: &mut V, cx: &mut EventCx) {
    let s = lens(v);
    s.touch();
    if let Some((at, timer)) = s.pending_click.take() {
        if at.elapsed() <= DOUBLE_CLICK {
            cx.cancel_timer(timer);
            cx.window_command(WindowCommand::ToggleFullscreen);
            return;
        }
    }
    let l = lens.clone();
    let timer = cx.spawn_timer::<V>(DOUBLE_CLICK, move |v, cx| {
        let s = l(v);
        s.pending_click = None;
        s.player.toggle_pause();
        cx.notify();
    });
    s.pending_click = Some((Instant::now(), timer));
}

fn message<V: 'static>(msg: String) -> Element<V> {
    div().p(16.0).rounded(8.0).bg(rgba(0x0000_00b3)).text_color(rgba(0xffff_ffff)).text_size(14.0).child(text(msg))
}

impl<V: 'static> Component<V> for VideoPlayer<'_, V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let (id, lens) = (self.id, self.lens.clone());
        let frame = self.state.player.frame().or_else(|| self.state.player.frame_source());
        let st = self.state.player.state();
        let playing = st.status == Status::Playing;
        let idle = cx.now().saturating_duration_since(self.state.last_activity);
        let show = !playing || idle < HIDE_AFTER;
        if show && playing {
            cx.request_redraw_in(HIDE_AFTER - idle);
        }
        self.state.controls_visible = show;
        let presence = cx.presence(controls_id(id), show, FADE);

        let center: Option<Element<V>> = match &st.status {
            Status::Error(e) => Some(message(e.clone())),
            Status::Loading if frame.is_none() => Some(message("Carregando…".into())),
            Status::Idle => Some(message("Nenhum vídeo".into())),
            _ => None,
        };
        let picture = frame.map(|f| img(f).absolute().inset(0.0).object_fit(self.fit));
        let lc = lens.clone();
        let surface = div()
            .id(surface_id(id))
            .absolute()
            .inset(0.0)
            .items_center()
            .justify_center()
            .on_click(move |v: &mut V, _, cx| on_surface_click(&lc, v, cx))
            .children(picture)
            .children(center);
        let bar = presence.map(|p| controls::bar(id, &lens, &mut *self.state, &st, cx).opacity(p));

        let (lk, lm) = (lens.clone(), lens);
        div()
            .track_focus(id)
            .relative()
            .overflow_hidden()
            .w_full()
            .h_full()
            .bg(rgba(0x0000_00ff))
            .on_key_down(move |v: &mut V, ev, cx| on_key(&lk, v, ev, cx))
            .on_mouse_move(move |v: &mut V, _, cx| {
                if lm(v).touch() {
                    cx.notify();
                }
            })
            .child(surface)
            .children(bar)
    }
}
