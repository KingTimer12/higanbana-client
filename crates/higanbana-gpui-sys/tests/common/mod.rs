#![allow(dead_code)]
use higanbana_gpui_sys::*;
use std::ffi::CString;
use std::ptr;

pub const OK: i32 = HG_OK as i32;

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

