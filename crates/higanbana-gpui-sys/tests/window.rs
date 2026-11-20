use higanbana_gpui_sys::*;
use std::ffi::CString;
use std::ptr;

const OK: i32 = HG_OK as i32;

pub struct Fixture {
    pub ctx: *mut hg_context,
    pub win: *mut hg_window,
    pub dl: *mut hg_draw_list,
}

impl Fixture {
    pub fn new() -> Self {
        unsafe {
            let mut ctx = ptr::null_mut();
            assert_eq!(hg_context_create(hg_backend_kind_HG_BACKEND_D3D11, &mut ctx), OK);
            let title = CString::new("teste").unwrap();
            let desc = hg_window_desc { title_utf8: title.as_ptr(), width: 320, height: 240, resizable: 1 };
            let mut win = ptr::null_mut();
            assert_eq!(hg_window_create(ctx, &desc, &mut win), OK, "{:?}", std::ffi::CStr::from_ptr(hg_last_error()));
            Fixture { ctx, win, dl: hg_draw_list_create() }
        }
    }

    pub fn frame(&self) -> [i32; 3] {
        let clear = hg_color { r: 0.1, g: 0.1, b: 0.12, a: 1.0 };
        unsafe { [hg_frame_begin(self.win, clear), hg_frame_submit(self.win, self.dl), hg_frame_end(self.win, 0)] }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe {
            hg_draw_list_destroy(self.dl);
            hg_window_destroy(self.win);
            hg_context_destroy(self.ctx);
        }
    }
}

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
