//! Integração com GPU: árvore -> layout -> draw list -> pixels lidos da janela.
use higanbana_gpui::app::GpuMeasurer;
use higanbana_gpui::paint::{GpuCanvas, ImageCache};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{enumerate, size, Context, DrawList, FontSpec, TextStyle, Tree, Window};

struct V;

const RED: u32 = 0xff0000;
const GREEN: u32 = 0x00ff00;
const BLUE: u32 = 0x0000ff;

fn setup() -> Context {
    Context::new(BackendKind::D3D11).expect("contexto")
}

fn window(ctx: &Context) -> Window<'_> {
    let opts = WindowOptions { title: "teste".into(), size: size(320.0, 240.0), resizable: false };
    Window::new(ctx, &opts).expect("janela")
}

/// Rebuild + paint + present; devolve os pixels nos pontos LÓGICOS pedidos.
fn render(ctx: &Context, win: &Window<'_>, root: Element<V>, points: &[(f32, f32)]) -> Vec<u32> {
    let scale = win.scale();
    let mut tree = Tree::new();
    let base = TextStyle { font: FontSpec::default(), size: 14.0, color: Color::WHITE, wrap: true };
    tree.rebuild(root, &base, win.logical_size(), &mut GpuMeasurer { ctx, scale });
    let mut images = ImageCache::new(ctx);
    images.prepare(&tree);
    let mut list = DrawList::new();
    let mut canvas = GpuCanvas::new(&mut list, ctx, &images, scale);
    tree.paint(&mut canvas);
    assert!(canvas.error.is_none(), "{:?}", canvas.error);
    let phys: Vec<_> = points.iter().map(|&(x, y)| ((x * scale) as u32, (y * scale) as u32)).collect();
    win.present_and_read(Color::BLACK, &list, &phys).expect("present")
}

fn boxes() -> Element<V> {
    div()
        .bg(rgb(0x0000ff))
        .p(10.0)
        .gap(10.0)
        .child(div().w(40.0).h(40.0).bg(rgb(0xff0000)))
        .child(div().w(40.0).h(40.0).bg(rgb(0x00ff00)).rounded(4.0))
}

#[test]
fn layout_is_drawn_at_logical_positions() {
    let ctx = setup();
    let win = window(&ctx);
    let px = render(&ctx, &win, boxes(), &[(30.0, 30.0), (80.0, 30.0), (5.0, 5.0), (200.0, 200.0)]);
    assert_eq!(px, vec![RED, GREEN, BLUE, BLUE]);
}

#[test]
fn image_element_and_overflow_clip() {
    let ctx = setup();
    let win = window(&ctx);
    let green = ImageData::from_rgba8(4, 4, [0u8, 255, 0, 255].repeat(16));
    let root = div()
        .child(img(green).w(40.0).h(40.0))
        .child(div().w(30.0).h(30.0).overflow_hidden().child(div().w(80.0).h(80.0).flex_shrink_0().bg(rgb(0xff0000))));
    let px = render(&ctx, &win, root, &[(20.0, 20.0), (50.0, 10.0), (90.0, 10.0)]);
    assert_eq!(px, vec![GREEN, RED, 0x000000]);
}

#[test]
fn quad_covers_parent_text_via_layer() {
    let ctx = setup();
    let win = window(&ctx);
    let root = div()
        .child(text("█").text_size(96.0))
        .child(div().absolute().top(0.0).left(0.0).w(120.0).h(120.0).bg(rgb(0x0000ff)));
    let px = render(&ctx, &win, root, &[(30.0, 60.0)]);
    assert_eq!(px, vec![BLUE]);
}

#[test]
fn switch_backend_at_runtime_keeps_drawing() {
    let ctx = setup();
    let win = window(&ctx);
    for info in enumerate() {
        ctx.switch_backend(info.kind).unwrap_or_else(|e| panic!("{:?}: {e}", info.kind));
        assert_eq!(ctx.backend(), info.kind);
        let px = render(&ctx, &win, boxes(), &[(30.0, 30.0), (80.0, 30.0)]);
        assert_eq!(px, vec![RED, GREEN], "{:?}", info.kind);
    }
    ctx.switch_backend(BackendKind::D3D11).unwrap();
}
