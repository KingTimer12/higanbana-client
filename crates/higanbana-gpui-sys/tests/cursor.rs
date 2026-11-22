use higanbana_gpui_sys::*;
mod common;
use common::{Fixture, OK};

#[test]
fn cursor_none_accepted() {
    let f = Fixture::new();
    unsafe {
        assert_eq!(hg_window_set_cursor(f.win, hg_cursor_HG_CURSOR_NONE), OK);
        assert_eq!(hg_window_cursor(f.win), hg_cursor_HG_CURSOR_NONE);
        assert_eq!(hg_window_set_cursor(f.win, hg_cursor_HG_CURSOR_ARROW), OK);
    }
}
