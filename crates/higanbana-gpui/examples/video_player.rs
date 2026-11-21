//! Player de vídeo: `cargo run -p higanbana-gpui --features video --example video_player -- <arquivo ou URL>`.
//! Sem argumento toca um padrão de teste do próprio mpv.
use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;
use higanbana_gpui::size;
use higanbana_gpui::video::Player;

const TEST_SOURCE: &str = "av://lavfi:testsrc2=size=1280x720:rate=30";

struct App1 {
    source: String,
    video: Option<VideoState>,
    error: Option<String>,
}

impl View for App1 {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        // O Player nasce no primeiro render: precisa do waker da janela.
        if self.video.is_none() && self.error.is_none() {
            match Player::new(cx.waker()) {
                Ok(p) => {
                    p.load(&self.source);
                    self.video = Some(VideoState::new(p));
                }
                Err(e) => self.error = Some(e),
            }
        }
        let title = self.source.rsplit(['/', '\\']).next().unwrap_or("Vídeo").to_string();
        let bar = TitleBar::new(title).render(cx);
        let body = match &mut self.video {
            Some(state) => VideoPlayer::new("player", state, |v: &mut App1| v.video.as_mut().expect("player criado")).render(cx),
            None => div().p(24.0).child(text(self.error.clone().unwrap_or_default())),
        };
        let theme = cx.theme();
        div()
            .flex_col()
            .w_full()
            .h_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(bar)
            .child(div().flex_1().min_h(0.0).child(body))
    }
}

fn main() {
    let source = std::env::args().nth(1).unwrap_or_else(|| TEST_SOURCE.to_string());
    let opts = WindowOptions { title: "higanbana · vídeo".into(), size: size(1280.0, 760.0), resizable: true, decorations: Decorations::Custom };
    let view = App1 { source, video: None, error: None };
    if let Err(e) = App::new().run(opts, view) {
        eprintln!("erro: {e}");
    }
}
