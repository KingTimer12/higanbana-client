//! Sonda de diagnóstico: abre janela no backend A, opcionalmente troca para B,
//! desenha um frame com texto e fica ocioso (wait_event_timeout) por N segundos.
//! Serve para medir CPU ociosa e RAM por backend com ferramenta externa.
//!
//! `cargo run -p higanbana-gpui-sys --example idle_probe -- d3d11 opengl d3d12 20`
//! (cadeia de backends; o último número = segundos ociosos)

use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{Duration, Instant};

const OK: i32 = HG_OK as i32;

fn check(r: i32, what: &str) {
    if r != OK {
        let msg = unsafe { CStr::from_ptr(hg_last_error()) }.to_string_lossy();
        panic!("{what}: {msg} (código {r})");
    }
}

#[repr(C)]
#[derive(Default)]
struct MemCounters {
    cb: u32, faults: u32, peak_ws: usize, ws: usize, qpp: usize, qp: usize, qnpp: usize, qnp: usize,
    pagefile: usize, peak_pagefile: usize, private: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> isize;
    fn K32GetProcessMemoryInfo(p: isize, c: *mut MemCounters, cb: u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn K32EnumProcessModules(p: isize, m: *mut isize, cb: u32, needed: *mut u32) -> i32;
    fn K32GetModuleBaseNameW(p: isize, m: isize, name: *mut u16, n: u32) -> u32;
}

/// Nomes dos módulos carregados no processo.
fn modules() -> Vec<String> {
    let mut m = vec![0isize; 1024];
    let mut need = 0u32;
    unsafe { K32EnumProcessModules(GetCurrentProcess(), m.as_mut_ptr(), (m.len() * 8) as u32, &mut need) };
    m[..need as usize / 8].iter().map(|&h| {
        let mut buf = [0u16; 260];
        let n = unsafe { K32GetModuleBaseNameW(GetCurrentProcess(), h, buf.as_mut_ptr(), 260) };
        String::from_utf16_lossy(&buf[..n as usize])
    }).collect()
}

/// Imprime working set e bytes privados do processo (MB) com um rótulo.
fn mem(label: &str) {
    let mut c = MemCounters { cb: size_of::<MemCounters>() as u32, ..Default::default() };
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    let mb = |b: usize| b as f64 / 1048576.0;
    println!("{label:<22} WS {:7.1} MB  privado {:7.1} MB", mb(c.ws), mb(c.private));
}

fn kind(s: &str) -> hg_backend_kind {
    match s {
        "d3d11" => hg_backend_kind_HG_BACKEND_D3D11,
        "d3d12" => hg_backend_kind_HG_BACKEND_D3D12,
        "vulkan" => hg_backend_kind_HG_BACKEND_VULKAN,
        "opengl" => hg_backend_kind_HG_BACKEND_OPENGL,
        "wgpu" => hg_backend_kind_HG_BACKEND_WGPU,
        _ => panic!("backend desconhecido: {s}"),
    }
}

fn frame(win: *mut hg_window, dl: *mut hg_draw_list, font: hg_font_id) {
    unsafe {
        hg_draw_list_reset(dl);
        let q = hg_quad { rect: hg_rect { x: 20.0, y: 20.0, w: 200.0, h: 80.0 },
                          fill: hg_color { r: 0.2, g: 0.4, b: 0.9, a: 1.0 }, ..std::mem::zeroed() };
        check(hg_draw_quad(dl, &q), "quad");
        let t = "Olá, sonda";
        check(hg_draw_text(dl, font, 18.0, 30.0, 40.0, 0.0, hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
                           t.as_ptr().cast(), t.len()), "text");
        check(hg_frame_begin(win, hg_color { r: 0.1, g: 0.1, b: 0.12, a: 1.0 }), "begin");
        mem("  frame_begin");
        check(hg_frame_submit(win, dl), "submit");
        mem("  frame_submit");
        check(hg_frame_end(win, 1), "end");
        mem("  frame_end");
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let secs: u64 = args.last().and_then(|s| s.parse().ok()).unwrap_or(20);
    if args.last().is_some_and(|s| s.parse::<u64>().is_ok()) { args.pop(); }
    let chain: Vec<hg_backend_kind> = args.iter().map(|s| kind(s)).collect();
    let first = chain.first().copied().unwrap_or(hg_backend_kind_HG_BACKEND_D3D11);
    #[cfg(feature = "wgpu")]
    wgpu_backend::install();
    mem("início");
    unsafe {
        let mut infos: [hg_backend_info; 8] = std::mem::zeroed();
        hg_enumerate_backends(infos.as_mut_ptr(), 8);
        mem("probe dos backends");
        let mut ctx = ptr::null_mut();
        check(hg_context_create(first, &mut ctx), "context_create");
        mem("contexto (+probe)");
        let title = CString::new("idle_probe").unwrap();
        let desc = hg_window_desc { title_utf8: title.as_ptr(), width: if std::env::var_os("PROBE_SMALL").is_some() { 320 } else { 1280 },
                                    height: if std::env::var_os("PROBE_SMALL").is_some() { 200 } else { 800 },
                                    resizable: 1 };
        let mut win = ptr::null_mut();
        check(hg_window_create(ctx, &desc, &mut win), "window_create");
        mem("janela + swapchain");
        let dl = hg_draw_list_create();
        let family = CString::new("Segoe UI").unwrap();
        let mut font = 0;
        check(hg_font_load(ctx, family.as_ptr(), 400.0, 0, &mut font), "font");
        mem("fonte");
        if std::env::var_os("PROBE_EMPTY_FIRST").is_some() {
            let before = modules();
            check(hg_frame_begin(win, hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), "begin");
            check(hg_frame_submit(win, dl), "submit");
            check(hg_frame_end(win, 1), "end");
            mem("frame vazio (present)");
            let new: Vec<_> = modules().into_iter().filter(|m| !before.contains(m)).collect();
            println!("  módulos novos no present: {new:?}");
        }
        frame(win, dl, font);
        mem("1º frame");
        for &k in chain.iter().skip(1) {
            std::thread::sleep(Duration::from_millis(500));
            check(hg_context_switch_backend(ctx, k), "switch");
            frame(win, dl, font);
            mem("após troca");
        }
        std::thread::sleep(Duration::from_secs(2));
        mem("ocioso 2s");
        println!("PID {} pronto, ocioso por {secs}s", std::process::id());
        let end = Instant::now() + Duration::from_secs(secs);
        let mut ev: hg_event = std::mem::zeroed();
        while Instant::now() < end {
            check(hg_window_wait_event_timeout(win, 250, &mut ev), "wait");
            if ev.kind == hg_event_kind_HG_EVENT_RESIZE { frame(win, dl, font); }
        }
        hg_draw_list_destroy(dl);
        hg_window_destroy(win);
        hg_context_destroy(ctx);
    }
}
