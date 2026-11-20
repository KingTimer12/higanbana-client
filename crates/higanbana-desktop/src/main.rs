// Sem console extra no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod avif;
mod catalog_state;
mod chrome;
mod components;
mod image_cache;
mod interact;
mod logging;
mod mem;
mod motion;
mod music;
mod reader;
mod series;
mod settings_dialog;
mod state;
mod theme;
mod ui;
mod updater;
mod video;
mod views;

use std::{rc::Rc, sync::Arc};

use futures::channel::mpsc;
use gpui_kit::{App, AppContext, Bounds, TitlebarOptions, Window, WindowBounds, WindowOptions, component::TitleBar, px, size};
use higanbana_core::{Core, Event, Paths};

fn main() {
    let paths = Paths::system().expect("pastas do app");
    let _log = logging::init(&paths.cache);
    // Diagnóstico: memória do processo a cada 15 s, em qualquer tela.
    let _ = std::thread::Builder::new().name("mem-probe".into()).spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(15));
        tracing::debug!(processo_mb = mem::private_mb(), "memória");
    });
    tracing::info!(version = higanbana_core::version::current(), data = %paths.data.display(), cache = %paths.cache.display(), processo_mb = mem::private_mb(), "higanbana abrindo");
    let (tx, rx) = mpsc::unbounded::<Event>();
    let events = Arc::new(move |e| {
        let _ = tx.unbounded_send(e);
    });
    let core = Arc::new(Core::new(paths, events).expect("núcleo do higanbana"));
    tracing::info!(processo_mb = mem::private_mb(), "núcleo pronto");

    gpui_kit::application().with_assets(ui::Assets).run(move |cx: &mut App| {
        gpui_kit::init(cx);
        theme::install(cx);
        motion::sync_system_preference(cx);
        app::bind_keys(cx);
        state::install(core.clone(), cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        // Tamanhos da janela em pixels: limite físico da plataforma, não espaçamento da interface.
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // A `TitleBar` do kit desenha a barra e os botões da janela.
            titlebar: Some(TitlebarOptions { title: Some("higanbana".into()), ..TitleBar::title_bar_options() }),
            window_min_size: Some(size(px(960.), px(600.))),
            // Transparente: o vídeo (janela nativa embaixo) aparece onde nada é pintado.
            window_background: gpui_kit::WindowBackgroundAppearance::Transparent,
            ..TitleBar::window_options()
        };
        tracing::info!(processo_mb = mem::private_mb(), "abrindo janela");
        gpui_kit::open_window(options, cx, |window, cx| {
            let hwnd = hwnd(window);
            if let Some(h) = hwnd {
                round_corners(h);
            }
            core.music().attach_media_keys(hwnd);
            let surface = Rc::new(video_parent(window).and_then(video::Surface::new).expect("janela do vídeo"));
            let handle = window.window_handle();
            cx.new(|cx| app::AppView::new(core.clone(), surface, rx, handle, window, cx))
        })
        .expect("janela principal");
        tracing::info!(processo_mb = mem::private_mb(), "janela aberta");
        cx.on_window_closed(|cx, _| {
            tracing::info!("janela fechada; saindo");
            cx.quit()
        })
        .detach();
        cx.activate(true);
    });
}

/// HWND da janela (teclas de mídia / SMTC e vídeo no Windows).
fn hwnd(window: &Window) -> Option<usize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as usize),
        _ => None,
    }
}

/// Onde pendurar a superfície do vídeo: HWND no Windows, `NSView` do GPUI no macOS.
fn video_parent(window: &Window) -> Option<usize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as usize),
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as usize),
        _ => None,
    }
}

/// Cantos arredondados do Windows 11. Com fundo transparente e barra própria
/// o sistema não arredonda sozinho; aqui pedimos explicitamente (no Windows 10
/// a chamada só falha, sem efeito).
#[cfg(windows)]
fn round_corners(hwnd: usize) {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute};
    let pref = DWMWCP_ROUND;
    // SAFETY: HWND da nossa janela, ponteiro para um valor vivo do tamanho informado.
    unsafe {
        DwmSetWindowAttribute(hwnd as _, DWMWA_WINDOW_CORNER_PREFERENCE as u32, (&pref as *const i32).cast(), size_of::<i32>() as u32);
    }
}

#[cfg(not(windows))]
fn round_corners(_: usize) {}
