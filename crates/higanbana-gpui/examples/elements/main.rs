//! Tela de exemplo dos elementos base: cabeçalho, barra lateral rolável, cards com
//! imagem e texto, contador clicável, troca de backend e de tema em tempo real,
//! campo de texto, tooltip, diálogo modal (`HGUI_DEMO_OPEN=1` abre os dois) e um shader WGSL.
mod extras;
mod shader;

use higanbana_gpui::prelude::*;

struct Showcase {
    count: u32,
    selected: usize,
    covers: Vec<ImageSource>,
    search: TextInputState,
    tip: bool,
    dialog: bool,
    gradient: Option<Shader>,
}

const TITLES: [&str; 6] = ["Frieren", "Dandadan", "Kusuriya", "Mushishi", "Odd Taxi", "Ping Pong"];

/// Gradiente diagonal procedural (sem arquivos).
fn cover(seed: u32) -> ImageSource {
    let (w, h) = (96u32, 128u32);
    let hue = [(220, 38, 38), (234, 88, 12), (22, 163, 74), (37, 99, 235), (147, 51, 234), (219, 39, 119)][seed as usize % 6];
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = (x + y) as f32 / (w + h) as f32;
            let mix = |c: u8| (c as f32 * (1.0 - t * 0.7) + 20.0 * t) as u8;
            px.extend_from_slice(&[mix(hue.0), mix(hue.1), mix(hue.2), 255]);
        }
    }
    ImageData::from_rgba8(w, h, px)
}

fn button(label: String, theme: &Theme, primary: bool) -> Element<Showcase> {
    let (bg, fg) = if primary { (theme.primary, theme.primary_foreground) } else { (theme.secondary, theme.secondary_foreground) };
    let hover = if primary { theme.primary.alpha(0.85) } else { theme.accent.alpha(0.6) };
    div().px(12.0).py(6.0).rounded(theme.radius - 2.0).bg(bg).text_color(fg).font_weight(600).hover(move |s| s.bg(hover)).child(label)
}

impl Showcase {
    fn header(&self, cx: &RenderCx<'_, Self>) -> Element<Self> {
        let theme = cx.theme();
        let backends: Vec<BackendKind> = cx.backends().iter().map(|b| b.kind).collect();
        let current = cx.backend();
        let switcher = button(format!("Backend: {}", current.label()), theme, false).on_click(move |_, _, cx| {
            let i = backends.iter().position(|k| *k == current).map_or(0, |i| i + 1);
            if let Some(next) = backends.get(i % backends.len().max(1)) {
                cx.switch_backend(*next);
            }
        });
        let next_theme = theme.toggled();
        let toggle = button(if theme.dark { "Tema claro".into() } else { "Tema escuro".into() }, theme, false)
            .on_click(move |_, _, cx| cx.set_theme(next_theme.clone()));
        div()
            .h(56.0)
            .px(20.0)
            .items_center()
            .justify_between()
            .border(1.0, theme.border)
            .bg(theme.card)
            .child(div().items_center().gap(10.0).child(div().size(14.0).rounded_full().bg(theme.primary)).child(text("Higanbana GPUI").text_size(18.0).font_bold()))
            .child(div().gap(8.0).child(switcher).child(toggle))
    }

    fn sidebar(&self, theme: &Theme) -> Element<Self> {
        let items = (0..40).map(|i| {
            let active = i == self.selected;
            div()
                .id(("item", i))
                .px(12.0)
                .py(8.0)
                .rounded(6.0)
                .flex_shrink_0()
                .when(active, |e| e.bg(theme.accent).text_color(theme.accent_foreground))
                .hover(|s| s.bg(rgb(0x808080).alpha(0.15)))
                .on_click(move |v: &mut Showcase, _, cx| {
                    v.selected = i;
                    cx.notify();
                })
                .child(format!("Episódio {:02}", i + 1))
        });
        scroll("sidebar").w(220.0).p(8.0).gap(2.0).bg(theme.card).border(1.0, theme.border).text_color(theme.muted_foreground).children(items)
    }

    fn card(&self, i: usize, theme: &Theme) -> Element<Self> {
        div()
            .w(300.0)
            .p(12.0)
            .gap(12.0)
            .rounded(theme.radius)
            .bg(theme.card)
            .border(1.0, theme.border)
            .shadow(rgba(0x0000_0040), 12.0)
            .hover(|s| s.border_color(rgb(0xdc2626)))
            .child(img(self.covers[i].clone()).w(72.0).h(96.0).rounded(6.0).flex_shrink_0())
            .child(
                div()
                    .flex_col()
                    .flex_1()
                    .gap(6.0)
                    .child(text(TITLES[i]).text_size(16.0).font_bold().text_color(theme.card_foreground))
                    .child(text("Texto que quebra em várias linhas dentro do card, medido pelo DirectWrite.").text_color(theme.muted_foreground)),
            )
    }

    fn counter(&self, cx: &RenderCx<'_, Self>) -> Element<Self> {
        let theme = cx.theme();
        div()
            .flex_wrap()
            .p(16.0)
            .gap(16.0)
            .items_center()
            .rounded(theme.radius)
            .bg(theme.card)
            .border(1.0, theme.border)
            .child(text(format!("Cliques: {}", self.count)).text_size(20.0).font_bold())
            .child(button("+1".into(), theme, true).on_click(|v: &mut Showcase, _, cx| {
                v.count += 1;
                cx.notify();
            }))
            .child(text(format!("Selecionado: episódio {:02}", self.selected + 1)).text_color(theme.muted_foreground))
            .child(extras::form(self, cx))
    }
}

impl View for Showcase {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        let theme = cx.theme();
        let mut cards: Vec<_> = (0..TITLES.len()).map(|i| self.card(i, theme)).collect();
        cards.extend(shader::card(self.gradient.as_ref(), theme));
        let counter = self.counter(cx);
        let sidebar = self.sidebar(theme);
        let (bg, fg) = (theme.background, theme.foreground);
        let dialog = extras::dialog(self, cx);
        div()
            .flex_col()
            .bg(bg)
            .text_color(fg)
            .child(self.header(cx))
            .child(
                div().flex_1().child(sidebar).child(
                    scroll("main").flex_1().p(20.0).gap(16.0).child(counter).child(div().flex_wrap().gap(16.0).children(cards)),
                ),
            )
            .children(dialog)
    }
}

fn main() {
    let demo = std::env::var("HGUI_DEMO_OPEN").is_ok_and(|v| v == "1");
    let view = Showcase {
        count: 0,
        selected: 0,
        covers: (0..TITLES.len() as u32).map(cover).collect(),
        search: TextInputState::with_text(if demo { "葬送のフリーレン 😀" } else { "" }).placeholder("Buscar anime…"),
        tip: demo,
        dialog: demo,
        gradient: shader::gradient(),
    };
    let opts = WindowOptions { title: "higanbana-gpui · elementos".into(), size: size(1100.0, 720.0), resizable: true, ..Default::default() };
    if let Err(e) = App::new().show_stats(true).run(opts, view) {
        eprintln!("erro: {e}");
    }
}

use higanbana_gpui::size;
