//! Player na GPU (D3D11): o mpv desenha com OpenGL direto na textura que a UI mostra.
use higanbana_gpui::app::GpuMeasurer;
use higanbana_gpui::paint::{GpuCanvas, ImageCache};
use higanbana_gpui::prelude::*;
use higanbana_gpui::video::{Player, PlayerOptions, RenderPath};
use higanbana_gpui::{size, Context, DrawList, FontSpec, ObjectFit, TextStyle, Tree, Waker, Window};
use std::time::{Duration, Instant};

struct V;

/// Cor igual a `rgb` com folga de 8 por canal (conversão de cor do mpv).
fn near(px: u32, rgb: u32) -> bool {
    (0..3).all(|i| ((px >> (i * 8)) & 0xff).abs_diff((rgb >> (i * 8)) & 0xff) <= 8)
}

const RED: &str = "av://lavfi:color=c=red:size=64x48:rate=10";
const BLUE: &str = "av://lavfi:color=c=blue:size=64x48:rate=10";
/// Metade de cima vermelha, de baixo azul (confere a orientação).
const SPLIT: &str = "av://lavfi:color=c=red:size=64x24:rate=10[a];color=c=blue:size=64x24:rate=10[b];[a][b]vstack";

fn window(ctx: &Context) -> Window<'_> {
    let opts = WindowOptions { title: "teste".into(), size: size(160.0, 120.0), resizable: false, ..Default::default() };
    Window::new(ctx, &opts).expect("janela")
}

/// Desenha o quadro atual do player em 40x40 e devolve os pixels pedidos (lógicos).
fn draw(ctx: &Context, win: &Window<'_>, cache: &mut ImageCache<'_>, p: &mut Player, pts: &[(f32, f32)]) -> Option<Vec<u32>> {
    let src = p.frame().or_else(|| p.frame_source())?;
    let scale = win.scale();
    let mut tree = Tree::<V>::new();
    let base = TextStyle { font: FontSpec::default(), size: 14.0, color: Color::WHITE, wrap: true, max_lines: None };
    tree.rebuild(img(src).w(40.0).h(40.0).object_fit(ObjectFit::Fill), &base, win.logical_size(), &mut GpuMeasurer { ctx, scale });
    cache.prepare(&tree);
    let mut list = DrawList::new();
    let mut canvas = GpuCanvas::new(&mut list, ctx, cache, scale);
    tree.paint(&mut canvas);
    let pts: Vec<(u32, u32)> = pts.iter().map(|(x, y)| ((x * scale) as u32, (y * scale) as u32)).collect();
    Some(win.present_and_read(Color::BLACK, &list, &pts).expect("present").into_iter().map(|c| c & 0xffffff).collect())
}

/// Desenha até `ok` aceitar os pixels (ou estourar `secs`).
fn until(ctx: &Context, win: &Window<'_>, cache: &mut ImageCache<'_>, p: &mut Player, pts: &[(f32, f32)], secs: u64, ok: impl Fn(&Player, &[u32]) -> bool) -> Vec<u32> {
    let end = Instant::now() + Duration::from_secs(secs);
    let mut last = Vec::new();
    while Instant::now() < end {
        if let Some(px) = draw(ctx, win, cache, p, pts) {
            if ok(p, &px) {
                return px;
            }
            last = px;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("tempo esgotado; caminho {:?}, pixels {:06x?}", p.render_path(), last);
}

#[test]
fn gpu_mostra_lavfi_vermelho() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load(RED);
    until(&ctx, &win, &mut cache, &mut p, &[(20.0, 20.0)], 8, |p, px| p.render_path() == RenderPath::Gpu && near(px[0], 0xff0000));
}

#[test]
fn gpu_mantem_a_orientacao() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load(SPLIT);
    until(&ctx, &win, &mut cache, &mut p, &[(20.0, 5.0), (20.0, 35.0)], 8, |p, px| {
        p.render_path() == RenderPath::Gpu && near(px[0], 0xff0000) && near(px[1], 0x0000ff)
    });
}

#[test]
fn gpu_troca_de_arquivo_volta_a_ter_quadro() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load_at(RED, None);
    until(&ctx, &win, &mut cache, &mut p, &[(20.0, 20.0)], 8, |p, px| p.render_path() == RenderPath::Gpu && near(px[0], 0xff0000));
    p.load_at(BLUE, None);
    assert!(!p.state().first_frame);
    until(&ctx, &win, &mut cache, &mut p, &[(20.0, 20.0)], 8, |p, px| p.state().first_frame && near(px[0], 0x0000ff));
}

#[test]
fn sem_gpu_cai_no_sw() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let mut p = Player::with_options(Waker::noop(), PlayerOptions::default().gpu(false)).expect("libmpv");
    p.load(RED);
    until(&ctx, &win, &mut cache, &mut p, &[(20.0, 20.0)], 8, |_, px| near(px[0], 0xff0000));
    std::thread::sleep(Duration::from_millis(300));
    draw(&ctx, &win, &mut cache, &mut p, &[(20.0, 20.0)]);
    assert_eq!(p.render_path(), RenderPath::Cpu);
}

#[test]
fn backend_sem_d3d11_fica_no_sw() {
    let Ok(ctx) = Context::new(BackendKind::OpenGL) else { return };
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let mut p = Player::new(Waker::noop()).expect("libmpv");
    p.load(RED);
    until(&ctx, &win, &mut cache, &mut p, &[(20.0, 20.0)], 8, |_, px| near(px[0], 0xff0000));
    assert_eq!(p.render_path(), RenderPath::Cpu);
}
