//! Galeria dos componentes no estilo da documentação do shadcn/ui: barra lateral
//! com as seções, cabeçalho com tema e backend, rodapé com estatísticas.
//!
//! Variáveis de ambiente para capturas: `HGUI_GALLERY_SECTION=<índice>`,
//! `HGUI_GALLERY_STATE=dialog|select|toast|backend|popover`, `HGUI_GALLERY_THEME=light`,
//! `HGUI_GALLERY_BACKEND=vulkan|opengl|d3d11|d3d12|wgpu`.
mod sections;
mod shell;

use higanbana_gpui::components::*;
use higanbana_gpui::prelude::*;
use higanbana_gpui::size;

/// Estado inteiro da galeria (os componentes não guardam estado; tudo mora aqui).
pub struct Gallery {
    pub section: usize,
    pub backend_select: SelectState,
    pub name: TextInputState,
    pub email: TextInputState,
    pub password: TextInputState,
    pub invalid: TextInputState,
    pub disabled_input: TextInputState,
    pub terms: bool,
    pub news: CheckState,
    pub wifi: bool,
    pub airplane: bool,
    pub volume: f32,
    pub opacity: f32,
    pub dialog: bool,
    pub tab: usize,
    pub genre: SelectState,
    pub tip: TooltipState,
    pub tip_side: TooltipState,
    pub toasts: Toasts,
    pub notes: Toasts,
    pub cover: ImageSource,
    pub clicks: u32,
    pub anim_card: bool,
    pub anim_big: bool,
    pub popover: PopoverState,
    pub banner: CarouselState,
    pub fileira: CarouselState,
    pub menu: PopoverState,
    pub ordem: usize,
    pub aba: usize,
    pub subtitle: Option<usize>,
    pub quality: Option<usize>,
    pub progress: f32,
    pub side: SidebarState,
    pub cmd: CommandState,
    pub side_active: usize,
}

impl View for Gallery {
    fn render(&mut self, cx: &mut RenderCx<'_, Self>) -> Element<Self> {
        let theme = cx.theme();
        let (bg, fg) = (theme.background, theme.foreground);
        let header = shell::header(self, cx);
        let sidebar = shell::sidebar(self, cx);
        let footer = shell::footer(cx);
        let page = sections::page(self, cx);
        let dialog = sections::overlay::dialog(self, cx);
        let toaster = Toaster::new("toaster", &self.toasts, |v: &mut Gallery| &mut v.toasts).render(cx);
        let notifier = Notifier::new("avisos", &self.notes, |v: &mut Gallery| &mut v.notes).render(cx);
        let line = |c| div().h(1.0).flex_shrink_0().bg(c);
        let border = cx.theme().border;
        div()
            .flex_col()
            .bg(bg)
            .text_color(fg)
            .child(header)
            .child(line(border))
            .child(
                div()
                    .flex_1()
                    .min_h(0.0)
                    .child(sidebar)
                    .child(div().w(1.0).flex_shrink_0().bg(border))
                    .child(scroll("pagina").flex_1().px(40.0).py(28.0).gap(20.0).child(page)),
            )
            .child(line(border))
            .child(footer)
            .child(dialog)
            .child(toaster)
            .child(notifier)
    }
}

/// Gradiente radial listrado para o Avatar (procedural, sem arquivos).
fn cover() -> ImageSource {
    let n = 64u32;
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 22.0, y as f32 - 20.0);
            let t = ((dx * dx + dy * dy).sqrt() / 60.0).min(1.0);
            let stripe = if (x / 6 + y / 6) % 2 == 0 { 14.0 } else { 0.0 };
            px.extend_from_slice(&[(236.0 - 120.0 * t) as u8, (72.0 + 40.0 * t + stripe) as u8, (153.0 + 60.0 * t) as u8, 255]);
        }
    }
    ImageData::from_rgba8(n, n, px)
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default().to_lowercase()
}

fn backend_from_env() -> BackendKind {
    match env("HGUI_GALLERY_BACKEND").as_str() {
        "d3d11" => BackendKind::D3D11,
        "d3d12" => BackendKind::D3D12,
        "vulkan" => BackendKind::Vulkan,
        "opengl" => BackendKind::OpenGL,
        "wgpu" => BackendKind::Wgpu,
        _ => BackendKind::Auto,
    }
}

fn main() {
    let state = env("HGUI_GALLERY_STATE");
    let mut view = Gallery {
        section: env("HGUI_GALLERY_SECTION").parse().unwrap_or(0),
        backend_select: SelectState { open: state == "backend", ..SelectState::default() },
        name: TextInputState::with_text("Frieren"),
        email: TextInputState::default().placeholder("voce@exemplo.com"),
        password: TextInputState::with_text("himmel123"),
        invalid: TextInputState::with_text("email-sem-arroba"),
        disabled_input: TextInputState::with_text("Somente leitura"),
        terms: true,
        news: CheckState::Indeterminate,
        wifi: true,
        airplane: false,
        volume: 40.0,
        opacity: 0.75,
        dialog: state == "dialog",
        tab: 0,
        genre: SelectState { open: state == "select", ..SelectState::with_selected(Some(1)) },
        tip: TooltipState::default(),
        tip_side: TooltipState::default(),
        toasts: Toasts::new(),
        notes: Toasts::new(),
        cover: cover(),
        clicks: 0,
        anim_card: true,
        anim_big: false,
        popover: PopoverState { open: state == "popover" },
        banner: CarouselState::new(4).looping(true).auto_advance(std::time::Duration::from_secs(4)),
        fileira: CarouselState::new(12),
        menu: PopoverState::default(),
        ordem: 0,
        aba: 0,
        subtitle: Some(1),
        quality: Some(2),
        progress: 40.0,
        side: SidebarState::default(),
        cmd: CommandState::new(),
        side_active: 0,
    };
    view.tip.visible = state == "tooltip";
    if state == "toast" {
        sections::overlay::push_demo_toasts(&mut view.toasts, &mut view.notes);
    }
    let theme = if env("HGUI_GALLERY_THEME") == "light" { Theme::light() } else { Theme::dark() };
    let opts = WindowOptions { title: "higanbana-gpui · galeria".into(), size: size(1280.0, 800.0), resizable: true, decorations: Decorations::Custom, min_size: None };
    if let Err(e) = App::new().theme(theme).backend(backend_from_env()).run(opts, view) {
        eprintln!("erro: {e}");
    }
}
