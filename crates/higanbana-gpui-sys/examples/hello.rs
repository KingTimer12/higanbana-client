//! Exemplo mínimo do core C: lista backends, abre janela, desenha cards com
//! sombra e texto, e só redesenha quando algo muda.
//!
//! `cargo run -p higanbana-gpui-sys --example hello -- --backend d3d11|auto`

use higanbana_gpui_sys::*;
use std::ffi::{CStr, CString};
use std::ptr;

const OK: i32 = HG_OK as i32;

fn check(r: i32, what: &str) {
    if r != OK {
        let msg = unsafe { CStr::from_ptr(hg_last_error()) }.to_string_lossy();
        panic!("{what}: {msg} (código {r})");
    }
}

fn parse_backend() -> hg_backend_kind {
    let args: Vec<String> = std::env::args().collect();
    match args.iter().position(|a| a == "--backend").and_then(|i| args.get(i + 1)).map(String::as_str) {
        Some("d3d11") => hg_backend_kind_HG_BACKEND_D3D11,
        Some("d3d12") => hg_backend_kind_HG_BACKEND_D3D12,
        Some("vulkan") => hg_backend_kind_HG_BACKEND_VULKAN,
        Some("opengl") => hg_backend_kind_HG_BACKEND_OPENGL,
        Some("wgpu") => hg_backend_kind_HG_BACKEND_WGPU,
        _ => hg_backend_kind_HG_BACKEND_AUTO,
    }
}

fn list_backends() {
    let mut infos: [hg_backend_info; 8] = unsafe { std::mem::zeroed() };
    let n = unsafe { hg_enumerate_backends(infos.as_mut_ptr(), infos.len() as i32) };
    println!("backends disponíveis:");
    for info in &infos[..n as usize] {
        let name = unsafe { CStr::from_ptr(info.name.as_ptr()) }.to_string_lossy();
        println!("  - {name}");
    }
}

fn rgba(hex: u32, a: f32) -> hg_color {
    let c = |s: u32| ((hex >> s) & 0xff) as f32 / 255.0;
    hg_color { r: c(16), g: c(8), b: c(0), a }
}

struct Scene {
    font: hg_font_id,
    bold: hg_font_id,
}

const CARDS: [(&str, &str, u32); 3] = [
    ("葬送のフリーレン", "Frieren — 28 episódios", 0xe0435e),
    ("ぼっち・ざ・ろっく！", "Bocchi the Rock! — 12 episódios", 0xf5a524),
    ("薬屋のひとりごと", "Kusuriya no Hitorigoto — 24 episódios", 0x7c5cff),
];

fn text(dl: *mut hg_draw_list, font: hg_font_id, size: f32, x: f32, y: f32, color: hg_color, s: &str) {
    check(unsafe { hg_draw_text(dl, font, size, x, y, 0.0, color, s.as_ptr().cast(), s.len()) }, "hg_draw_text");
}

fn draw(scene: &Scene, dl: *mut hg_draw_list, width: f32, scale: f32) {
    unsafe { hg_draw_list_reset(dl) };
    text(dl, scene.bold, 28.0 * scale, 32.0 * scale, 24.0 * scale, rgba(0xffffff, 1.0), "Higanbana GPUI");
    let card_w = ((width - 32.0 * scale * 4.0) / 3.0).max(120.0 * scale);
    for (i, (title, subtitle, accent)) in CARDS.iter().enumerate() {
        let x = 32.0 * scale + i as f32 * (card_w + 32.0 * scale);
        let y = 96.0 * scale;
        let card = hg_quad {
            rect: hg_rect { x, y, w: card_w, h: 180.0 * scale },
            fill: rgba(0x1c1c22, 1.0),
            border: rgba(*accent, 0.6),
            radius: [16.0 * scale; 4],
            border_width: 1.0 * scale,
            shadow_blur: 18.0 * scale,
            shadow: rgba(0x000000, 0.55),
        };
        let stripe = hg_quad {
            rect: hg_rect { x: x + 16.0 * scale, y: y + 16.0 * scale, w: 48.0 * scale, h: 6.0 * scale },
            fill: rgba(*accent, 1.0),
            border: rgba(0, 0.0),
            radius: [3.0 * scale; 4],
            border_width: 0.0,
            shadow_blur: 0.0,
            shadow: rgba(0, 0.0),
        };
        unsafe {
            check(hg_draw_quad(dl, &card), "card");
            check(hg_draw_quad(dl, &stripe), "faixa");
        }
        text(dl, scene.bold, 20.0 * scale, x + 16.0 * scale, y + 40.0 * scale, rgba(0xffffff, 1.0), title);
        text(dl, scene.font, 14.0 * scale, x + 16.0 * scale, y + 76.0 * scale, rgba(0xa1a1aa, 1.0), subtitle);
    }
}

fn main() {
    list_backends();
    unsafe {
        let mut ctx = ptr::null_mut();
        check(hg_context_create(parse_backend(), &mut ctx), "hg_context_create");
        println!("usando backend: {}", hg_context_backend(ctx));

        let title = CString::new("Higanbana GPUI — hello").unwrap();
        let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 960, height: 600, resizable: 1 };
        let mut win = ptr::null_mut();
        check(hg_window_create(ctx, &desc, &mut win), "hg_window_create");

        let family = CString::new("Segoe UI").unwrap();
        let mut scene = Scene { font: 0, bold: 0 };
        check(hg_font_load(ctx, family.as_ptr(), 400.0, 0, &mut scene.font), "fonte");
        check(hg_font_load(ctx, family.as_ptr(), 700.0, 0, &mut scene.bold), "fonte negrito");

        let dl = hg_draw_list_create();
        let mut width = 960.0 * hg_window_dpi_scale(win);
        let mut frames = 0u64;
        let mut dirty = true;
        loop {
            if dirty {
                let scale = hg_window_dpi_scale(win);
                draw(&scene, dl, width, scale);
                check(hg_frame_begin(win, rgba(0x0f0f12, 1.0)), "frame_begin");
                check(hg_frame_submit(win, dl), "frame_submit");
                check(hg_frame_end(win, 1), "frame_end");
                frames += 1;
                if frames == 1 || frames % 120 == 0 {
                    let mut s: hg_stats = std::mem::zeroed();
                    hg_context_stats(ctx, &mut s);
                    println!(
                        "frame {frames}: {:.2} ms, gpu {} KiB, arena {} KiB, {} draw calls, {} glyphs",
                        s.frame_ms, s.gpu_bytes / 1024, s.arena_bytes / 1024, s.draw_calls, s.atlas_entries
                    );
                }
                dirty = false;
            }
            let mut ev: hg_event = std::mem::zeroed();
            check(hg_window_wait_event(win, &mut ev), "wait_event");
            match ev.kind {
                k if k == hg_event_kind_HG_EVENT_CLOSE => break,
                k if k == hg_event_kind_HG_EVENT_RESIZE => {
                    width = ev.u.resize.width as f32;
                    dirty = true;
                }
                _ => {}
            }
        }

        hg_draw_list_destroy(dl);
        hg_window_destroy(win);
        hg_context_destroy(ctx);
    }
}
