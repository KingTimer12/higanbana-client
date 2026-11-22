//! Imagem dinâmica (quadros de vídeo) no cache e o Waker entre threads.
use higanbana_gpui::app::GpuMeasurer;
use higanbana_gpui::paint::{GpuCanvas, ImageCache};
use higanbana_gpui::prelude::*;
use higanbana_gpui::{size, Context, DrawList, Event, FontSpec, TextStyle, Tree, Window};
use std::time::{Duration, Instant};

struct V;

fn window(ctx: &Context) -> Window<'_> {
    let opts = WindowOptions { title: "teste".into(), size: size(160.0, 120.0), resizable: false, ..Default::default() };
    Window::new(ctx, &opts).expect("janela")
}

fn solid(rgba: [u8; 4], w: u32, h: u32) -> Vec<u8> {
    (0..w * h).flat_map(|_| rgba).collect()
}

/// Prepara o cache com uma árvore que mostra `src` em 40x40 e devolve o pixel (20,20).
fn frame(ctx: &Context, win: &Window<'_>, cache: &mut ImageCache<'_>, src: &ImageSource) -> u32 {
    let scale = win.scale();
    let mut tree = Tree::<V>::new();
    let base = TextStyle { font: FontSpec::default(), size: 14.0, color: Color::WHITE, wrap: true, max_lines: None };
    tree.rebuild(img(src.clone()).w(40.0).h(40.0), &base, win.logical_size(), &mut GpuMeasurer { ctx, scale });
    cache.prepare(&tree);
    let mut list = DrawList::new();
    let mut canvas = GpuCanvas::new(&mut list, ctx, cache, scale);
    tree.paint(&mut canvas);
    let p = ((20.0 * scale) as u32, (20.0 * scale) as u32);
    win.present_and_read(Color::BLACK, &list, &[p]).expect("present")[0] & 0xffffff
}

#[test]
fn dynamic_image_update_in_place() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let src = ImageData::dynamic(4, 4);
    src.set_frame(4, 4, solid([255, 0, 0, 255], 4, 4));
    assert_eq!(frame(&ctx, &win, &mut cache, &src), 0xff0000);
    let before = cache.get(&src).unwrap().raw_ptr();
    src.set_frame(4, 4, solid([0, 0, 255, 255], 4, 4));
    assert_eq!(frame(&ctx, &win, &mut cache, &src), 0x0000ff);
    assert_eq!(cache.get(&src).unwrap().raw_ptr(), before, "mesma imagem do core, atualizada");
    assert_eq!(cache.len(), 1);
}

#[test]
fn dynamic_image_resize_recreates() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    let mut cache = ImageCache::new(&ctx);
    let src = ImageData::dynamic(4, 4);
    src.set_frame(4, 4, solid([255, 0, 0, 255], 4, 4));
    frame(&ctx, &win, &mut cache, &src);
    src.set_frame(8, 2, solid([0, 255, 0, 255], 8, 2));
    assert_eq!(frame(&ctx, &win, &mut cache, &src), 0x00ff00);
    assert_eq!(cache.get(&src).unwrap().size(), (8, 2));
    assert_eq!(src.size(), (8, 2));
}

#[test]
fn waker_wakes_window_from_thread() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let win = window(&ctx);
    while win.poll_event().is_some() {}
    let w = win.waker();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        w.wake();
    });
    let end = Instant::now() + Duration::from_secs(3);
    let mut got = false;
    while Instant::now() < end && !got {
        got = matches!(win.wait_event_timeout(Duration::from_millis(200)).unwrap(), Some(Event::Wake));
    }
    assert!(got);
}

#[test]
fn waker_noop_after_window_drop() {
    let ctx = Context::new(BackendKind::D3D11).unwrap();
    let w = {
        let win = window(&ctx);
        win.waker()
    };
    w.wake();
    std::thread::spawn(move || w.wake()).join().unwrap();
}
