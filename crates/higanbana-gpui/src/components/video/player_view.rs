//! Render do `VideoPlayer`: superfície, estados (carregando/erro), teclado e cliques.
use super::{controls, controls_id, surface_id, Lens, VideoPlayer, VideoSurface};
use crate::anim::Transition;
use crate::color::rgba;
use crate::components::Component;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, text, Element};
use crate::gfx::WindowCommand;
use crate::input::{Key, KeyEvent};
use crate::style::Styled;
use crate::video::Status;

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

fn message<V: 'static>(msg: String) -> Element<V> {
    div().p(16.0).rounded(8.0).bg(rgba(0x0000_00b3)).text_color(rgba(0xffff_ffff)).text_size(14.0).child(text(msg))
}

impl<V: 'static> Component<V> for VideoPlayer<'_, V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let (id, lens) = (self.id, self.lens.clone());
        let st = self.state.player.state();
        let playing = st.status == Status::Playing;
        let left = self.state.hide.tick(cx.now());
        let show = !self.state.hide.hidden(!playing || self.state.menu_open());
        if let (true, Some(left)) = (playing, left) {
            cx.request_redraw_in(left);
        }
        self.state.controls_visible = show;
        let presence = cx.presence(controls_id(id), show, FADE);

        let center: Option<Element<V>> = match &st.status {
            Status::Error(e) => Some(message(e.clone())),
            Status::Loading if self.state.player.frame_source().is_none() => Some(message("Carregando…".into())),
            Status::Idle => Some(message("Nenhum vídeo".into())),
            _ => None,
        };
        let (lg, lc) = (lens.clone(), lens.clone());
        let picture = VideoSurface::new(surface_id(id), &mut self.state.player, move |v: &mut V| &mut lg(v).gate)
            .fit(self.fit)
            .on_click(move |v: &mut V, cx| {
                let s = lc(v);
                s.touch();
                s.player.toggle_pause();
                cx.notify();
            })
            .on_double_click(|_, cx| cx.window_command(WindowCommand::ToggleFullscreen))
            .render(cx);
        let surface = div().absolute().inset(0.0).items_center().justify_center().child(picture).children(center);
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
                let s = lm(v);
                if std::mem::take(&mut s.want_focus) {
                    // Atalhos funcionam sem precisar clicar no vídeo antes.
                    cx.focus(id);
                }
                if s.touch() {
                    cx.notify();
                }
            })
            .child(surface)
            .children(bar)
    }
}
