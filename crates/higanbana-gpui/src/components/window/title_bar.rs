//! Barra de título desenhada pelo app (`Decorations::Custom`): arrasta a janela,
//! duplo clique maximiza, botões minimizar/maximizar/fechar no padrão do Windows.
use crate::color::{rgb, Color};
use crate::icon::icon;
use crate::components::Component;
use crate::cx::RenderCx;
use crate::element::{div, text, Element};
use crate::gfx::{HitKind, WindowCommand};
use crate::style::Styled;
use std::borrow::Cow;

pub const TITLE_BAR_HEIGHT: f32 = 40.0;
/// Altura da barra no visual do kit (`TitleBar::kit`).
pub const KIT_TITLE_BAR_HEIGHT: f32 = 34.0;
/// Largura dos botões de legenda do Windows.
pub const CAPTION_BUTTON_WIDTH: f32 = 46.0;
/// Glifos da fonte Segoe MDL2 Assets (Windows 10 e 11).
pub const GLYPH_MINIMIZE: &str = "\u{E921}";
pub const GLYPH_MAXIMIZE: &str = "\u{E922}";
pub const GLYPH_RESTORE: &str = "\u{E923}";
pub const GLYPH_CLOSE: &str = "\u{E8BB}";
const ICON_FONT: &str = "Segoe MDL2 Assets";
/// Vermelho do botão fechar do Windows.
const CLOSE_HOVER: crate::color::Color = rgb(0xC42B1C);

/// `TitleBar::new("App").leading(logo).content(abas).trailing(ações)`.
/// Em fullscreen não aparece.
pub struct TitleBar<V> {
    title: Cow<'static, str>,
    leading: Option<Element<V>>,
    content: Option<Element<V>>,
    trailing: Option<Element<V>>,
    kit: bool,
}

impl<V> TitleBar<V> {
    pub fn new(title: impl Into<Cow<'static, str>>) -> Self {
        Self { title: title.into(), leading: None, content: None, trailing: None, kit: false }
    }

    /// Visual do gpui-kit: 34 px, borda embaixo, fundo `title_bar` misturado ao
    /// fundo e botões da janela quadrados com ícones Lucide.
    pub fn kit(mut self) -> Self {
        self.kit = true;
        self
    }

    /// À esquerda do título (logo/ícone).
    pub fn leading(mut self, e: impl Into<Element<V>>) -> Self {
        self.leading = Some(e.into());
        self
    }

    /// Centro da barra, ocupa o espaço livre (abas, busca). Área vazia continua arrastando.
    pub fn content(mut self, e: impl Into<Element<V>>) -> Self {
        self.content = Some(e.into());
        self
    }

    /// Logo antes dos botões da janela (ações, seletor).
    pub fn trailing(mut self, e: impl Into<Element<V>>) -> Self {
        self.trailing = Some(e.into());
        self
    }
}

fn caption_button<V: 'static>(glyph: &'static str, hover_bg: crate::color::Color, hover_fg: crate::color::Color) -> Element<V> {
    div()
        .w(CAPTION_BUTTON_WIDTH)
        .h(TITLE_BAR_HEIGHT)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .child(text(glyph).font_family(ICON_FONT).text_size(10.0))
}

fn command<V: 'static>(e: Element<V>, c: WindowCommand) -> Element<V> {
    e.on_click(move |_: &mut V, _, cx| cx.window_command(c))
}

/// Botão da janela do kit: quadrado de 34 px com ícone de 14 px.
fn kit_button<V: 'static>(name: &'static str, fg: Color, hover_bg: Color, hover_fg: Color) -> Element<V> {
    div()
        .w(KIT_TITLE_BAR_HEIGHT)
        .h_full()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .text_color(fg)
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .child(icon(name).size(14.0))
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color { r: a.r * t + b.r * (1.0 - t), g: a.g * t + b.g * (1.0 - t), b: a.b * t + b.b * (1.0 - t), a: a.a * t + b.a * (1.0 - t) }
}

impl<V: 'static> TitleBar<V> {
    fn render_kit(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let state = cx.window_state();
        let t = cx.theme();
        let fg = t.foreground;
        let (hb, hf) = (t.secondary_hover, t.secondary_foreground);
        let max_icon = if state.maximized { "window-restore" } else { "window-maximize" };
        let buttons = div()
            .flex_shrink_0()
            .h_full()
            .child(command(kit_button("window-minimize", fg, hb, hf), WindowCommand::Minimize))
            .child(command(kit_button(max_icon, fg, hb, hf), WindowCommand::ToggleMaximize).window_control(HitKind::Maximize))
            .child(command(kit_button("window-close", fg, t.destructive, t.destructive_foreground), WindowCommand::Close));
        let mut bar = div().flex_1().min_w(0.0).h_full().items_center().pl(12.0).children(self.leading);
        if !self.title.is_empty() {
            bar = bar.child(text(self.title).text_size(t.typography.sm).font_weight(500).whitespace_nowrap());
        }
        bar = bar.children(self.content).children(self.trailing);
        div()
            .w_full()
            .h(KIT_TITLE_BAR_HEIGHT)
            .flex_shrink_0()
            .flex_col()
            .bg(mix(t.title_bar, t.background, 0.55))
            .text_color(fg)
            .window_drag()
            .child(div().w_full().flex_1().min_h(0.0).items_center().child(bar).child(buttons))
            .child(div().w_full().h(1.0).flex_shrink_0().bg(t.title_bar_border))
    }
}

impl<V: 'static> Component<V> for TitleBar<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let state = cx.window_state();
        if state.fullscreen {
            return div().hidden();
        }
        if self.kit {
            return self.render_kit(cx);
        }
        let theme = cx.theme();
        let (fg, muted) = (theme.foreground, theme.muted_foreground);
        let hover = theme.accent;
        let title_color = if state.focused { fg } else { muted };
        let max_glyph = if state.maximized { GLYPH_RESTORE } else { GLYPH_MAXIMIZE };
        let buttons = div()
            .flex_shrink_0()
            .child(command(caption_button(GLYPH_MINIMIZE, hover, fg), WindowCommand::Minimize))
            .child(command(caption_button(max_glyph, hover, fg), WindowCommand::ToggleMaximize).window_control(HitKind::Maximize))
            .child(command(caption_button(GLYPH_CLOSE, CLOSE_HOVER, rgb(0xFFFFFF)), WindowCommand::Close));
        div()
            .w_full()
            .h(TITLE_BAR_HEIGHT)
            .flex_shrink_0()
            .items_center()
            .bg(theme.background)
            .text_color(title_color)
            .window_drag()
            .child(
                div()
                    .flex_1()
                    .min_w(0.0)
                    .h(TITLE_BAR_HEIGHT)
                    .items_center()
                    .gap(10.0)
                    .pl(12.0)
                    .children(self.leading)
                    .child(text(self.title).text_size(theme.typography.sm).font_weight(500).whitespace_nowrap())
                    .child(div().flex_1().min_w(0.0).h(TITLE_BAR_HEIGHT).items_center().justify_center().children(self.content))
                    .children(self.trailing.map(|t| div().items_center().pr(8.0).child(t))),
            )
            .child(buttons)
    }
}
