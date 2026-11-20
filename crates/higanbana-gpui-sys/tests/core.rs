use higanbana_gpui_sys::*;
use std::ffi::CStr;

fn quad() -> hg_quad {
    hg_quad {
        rect: hg_rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
        fill: hg_color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
        border: hg_color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
        radius: [2.0; 4],
        border_width: 0.0,
        shadow_blur: 0.0,
        shadow: hg_color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
    }
}

#[test]
fn draw_list_counts_quads() {
    unsafe {
        let dl = hg_draw_list_create();
        for _ in 0..3 {
            assert_eq!(hg_draw_quad(dl, &quad()), HG_OK as i32);
        }
        assert_eq!(hg_draw_list_quad_count(dl), 3);
        hg_draw_list_reset(dl);
        assert_eq!(hg_draw_list_quad_count(dl), 0);
        hg_draw_list_destroy(dl);
    }
}

#[test]
fn draw_list_grows_past_first_block() {
    unsafe {
        let dl = hg_draw_list_create();
        for _ in 0..10_000 {
            assert_eq!(hg_draw_quad(dl, &quad()), HG_OK as i32);
        }
        assert_eq!(hg_draw_list_quad_count(dl), 10_000);
        hg_draw_list_destroy(dl);
    }
}

#[test]
fn clip_pop_on_empty_stack_is_noop() {
    unsafe {
        let dl = hg_draw_list_create();
        hg_pop_clip(dl);
        assert_eq!(hg_push_clip(dl, hg_rect { x: 0.0, y: 0.0, w: 5.0, h: 5.0 }), HG_OK as i32);
        hg_pop_clip(dl);
        hg_pop_clip(dl);
        hg_draw_list_destroy(dl);
    }
}

#[test]
fn null_quad_is_invalid() {
    unsafe {
        let dl = hg_draw_list_create();
        assert_eq!(hg_draw_quad(dl, std::ptr::null()), HG_ERR_INVALID as i32);
        let msg = CStr::from_ptr(hg_last_error()).to_str().unwrap();
        assert!(!msg.is_empty());
        hg_draw_list_destroy(dl);
    }
}
