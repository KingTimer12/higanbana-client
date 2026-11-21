use higanbana_gpui_sys::*;
mod common;
use common::{Fixture, OK};

fn quad(i: usize) -> hg_quad {
    hg_quad {
        rect: hg_rect { x: (i % 10) as f32 * 30.0, y: (i / 10) as f32 * 22.0, w: 26.0, h: 18.0 },
        fill: hg_color { r: 0.9, g: 0.3, b: 0.4, a: 1.0 },
        border: hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
        radius: [6.0, 6.0, 6.0, 6.0],
        border_width: 1.0,
        shadow_blur: 4.0,
        shadow: hg_color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
    }
}

#[test]
fn window_renders_quads_frames() {
    let f = Fixture::new();
    for _ in 0..3 {
        unsafe {
            hg_draw_list_reset(f.dl);
            assert_eq!(hg_push_clip(f.dl, hg_rect { x: 0.0, y: 0.0, w: 200.0, h: 200.0 }), OK);
            for i in 0..100 {
                assert_eq!(hg_draw_quad(f.dl, &quad(i)), OK);
            }
            hg_pop_clip(f.dl);
        }
        assert_eq!(f.frame(), [OK; 3]);
    }
}

#[test]
fn zero_size_resize_is_noop() {
    let f = Fixture::new();
    unsafe {
        hg__window_inject_resize(f.win, 0, 0);
        hg_draw_quad(f.dl, &quad(0));
    }
    assert_eq!(f.frame(), [OK; 3]);
    unsafe { hg__window_inject_resize(f.win, 400, 300) };
    assert_eq!(f.frame(), [OK; 3]);
}

#[test]
fn close_event_after_request() {
    let f = Fixture::new();
    unsafe {
        hg__window_request_close(f.win);
        let mut ev: hg_event = std::mem::zeroed();
        let mut saw_close = false;
        while hg_window_poll_event(f.win, &mut ev) != 0 {
            saw_close |= ev.kind == hg_event_kind_HG_EVENT_CLOSE;
        }
        assert!(saw_close);
    }
}

#[test]
fn stats_after_frame_have_draw_calls() {
    let f = Fixture::new();
    let family = std::ffi::CString::new("Segoe UI").unwrap();
    let mut font = 0;
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    unsafe {
        assert_eq!(hg_font_load(f.ctx, family.as_ptr(), 400.0, 0, &mut font), OK);
        hg_draw_quad(f.dl, &quad(0));
        let text = b"stats";
        assert_eq!(hg_draw_text(f.dl, font, 16.0, 4.0, 4.0, 0.0, white, text.as_ptr().cast(), text.len()), OK);
    }
    assert_eq!(f.frame(), [OK; 3]);
    let mut s: hg_stats = unsafe { std::mem::zeroed() };
    unsafe { hg_context_stats(f.ctx, &mut s) };
    assert!(s.draw_calls >= 2, "draw_calls = {}", s.draw_calls);
    assert!(s.arena_bytes > 0);
    assert!(s.gpu_bytes > 0);
    assert!(s.atlas_entries > 0);
}
