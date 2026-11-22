// Sem console extra no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Migração (Planos 3 e 4): motion, imagens e estado já trazem o que as telas vão usar.
#![allow(dead_code)]

mod avif;
mod chrome;
mod image_cache;
mod login;
mod logging;
mod mem;
mod motion;
mod pages;
mod series;
mod views;
mod root;
mod state;
mod theme;
mod ui;
mod video;
// Em migração para a higanbana-gpui (Planos 3 e 4): fora do build até lá.
// mod app; mod catalog_state; mod music;
mod reader;
// mod settings_dialog; mod updater;

use std::sync::Arc;

use higanbana_core::{Core, Event, Paths};
use higanbana_gpui::{size, App, Decorations, WindowOptions};

fn main() {
    let paths = Paths::system().expect("pastas do app");
    let _log = logging::init(&paths.cache);
    // Diagnóstico: memória do processo a cada 15 s, em qualquer tela.
    let _ = std::thread::Builder::new().name("mem-probe".into()).spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(15));
        tracing::debug!(processo_mb = mem::private_mb(), "memória");
    });
    tracing::info!(version = higanbana_core::version::current(), data = %paths.data.display(), cache = %paths.cache.display(), processo_mb = mem::private_mb(), "higanbana abrindo");

    let mut app = App::new().theme(theme::load());
    // Eventos do núcleo chegam em outras threads: viram mensagens para a view.
    let mailbox = app.mailbox::<root::Higanbana>();
    let mb = mailbox.clone();
    let events = Arc::new(move |e: Event| {
        mb.send(move |v: &mut root::Higanbana, cx| v.on_core_event(e, cx));
    });
    // Resultados de tarefas do núcleo voltam ao estado na thread da UI.
    let post: state::Post = Arc::new(move |job: state::Job| {
        mailbox.send(move |v: &mut root::Higanbana, cx| {
            job(&mut v.state, cx);
            cx.notify();
        });
    });
    let core = Arc::new(Core::new(paths, events).expect("núcleo do higanbana"));
    tracing::info!(processo_mb = mem::private_mb(), "núcleo pronto");

    let options = WindowOptions {
        title: "higanbana".into(),
        size: size(1280.0, 800.0),
        resizable: true,
        decorations: Decorations::Custom,
        // Tamanho mínimo em pixels: limite físico da plataforma, não espaçamento da interface.
        min_size: Some(size(960.0, 600.0)),
    };
    let media = core.clone();
    let app = app.on_window(move |hwnd| {
        round_corners(hwnd);
        media.music().attach_media_keys(Some(hwnd));
    });
    tracing::info!(processo_mb = mem::private_mb(), "abrindo janela");
    if let Err(e) = app.run(options, root::Higanbana::new(core, post)) {
        tracing::error!(erro = %e, "janela principal");
    }
    tracing::info!("janela fechada; saindo");
}

/// Cantos arredondados do Windows 11 (no Windows 10 a chamada só falha, sem efeito).
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
