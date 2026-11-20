use higanbana_gpui_sys::*;
use std::ffi::CString;
mod common;
use common::{Fixture, OK};

fn load_font(ctx: *mut hg_context) -> hg_font_id {
    let family = CString::new("Segoe UI").unwrap();
    let mut id = 0;
    assert_eq!(unsafe { hg_font_load(ctx, family.as_ptr(), 400.0, 0, &mut id) }, OK);
    id
}

fn measure(ctx: *mut hg_context, font: hg_font_id, text: &[u8], max_width: f32) -> (i32, hg_text_metrics) {
    let mut m = hg_text_metrics { width: 0.0, height: 0.0, baseline: 0.0 };
    let r = unsafe { hg_text_measure(ctx, font, 16.0, text.as_ptr().cast(), text.len(), max_width, &mut m) };
    (r, m)
}

#[test]
fn measure_ascii_nonzero_and_monotonic() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    let (r1, one) = measure(f.ctx, font, b"A", 0.0);
    let (r4, four) = measure(f.ctx, font, b"AAAA", 0.0);
    assert_eq!((r1, r4), (OK, OK));
    assert!(one.width > 0.0 && four.width > one.width);
    assert!(one.baseline > 0.0 && one.baseline <= one.height);
}

#[test]
fn measure_japanese_title() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    let (r, m) = measure(f.ctx, font, "葬送のフリーレン".as_bytes(), 0.0);
    assert_eq!(r, OK);
    assert!(m.width > 0.0);
}

#[test]
fn wraps_with_max_width() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    let text = b"um texto bem comprido que precisa quebrar em varias linhas";
    let (_, single) = measure(f.ctx, font, text, 0.0);
    let (r, wrapped) = measure(f.ctx, font, text, 100.0);
    assert_eq!(r, OK);
    assert!(wrapped.height > single.height * 1.5);
}

#[test]
fn invalid_utf8_is_invalid_not_crash() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    assert_eq!(measure(f.ctx, font, &[0xff, 0xfe], 0.0).0, HG_ERR_INVALID as i32);
    let color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let bad = [0xffu8, 0xfe];
    let r = unsafe { hg_draw_text(f.dl, font, 16.0, 0.0, 0.0, 0.0, color, bad.as_ptr().cast(), bad.len()) };
    assert_eq!(r, HG_ERR_INVALID as i32);
}

#[test]
fn draws_text_in_window() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    let color = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let text = "Higanbana — 葬送のフリーレン".as_bytes();
    for _ in 0..3 {
        unsafe {
            hg_draw_list_reset(f.dl);
            assert_eq!(hg_draw_text(f.dl, font, 18.0, 10.0, 10.0, 300.0, color, text.as_ptr().cast(), text.len()), OK);
        }
        assert_eq!(f.frame(), [OK; 3]);
    }
}

fn draw_and_read(f: &Fixture, px: (u32, u32)) -> ([i32; 2], u32) {
    let clear = hg_color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut pixel = 0u32;
    unsafe {
        let r1 = hg_frame_begin(f.win, clear);
        let r2 = hg_frame_submit(f.win, f.dl);
        hg__window_read_pixel(f.win, px.0, px.1, &mut pixel);
        hg_frame_end(f.win, 0);
        ([r1, r2], pixel)
    }
}

#[test]
fn text_only_first_frame_is_visible() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let block = "█";
    unsafe {
        assert_eq!(hg_push_clip(f.dl, hg_rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 }), OK);
        assert_eq!(hg_draw_text(f.dl, font, 96.0, 10.0, 10.0, 0.0, white, block.as_ptr().cast(), block.len()), OK);
    }
    let (rs, pixel) = draw_and_read(&f, (40, 70));
    assert_eq!(rs, [OK; 2]);
    assert!(pixel & 0x00ff_ffff > 0x0080_8080, "pixel = {pixel:#010x}");
}

#[test]
fn full_glyph_atlas_skips_glyphs_instead_of_failing_frame() {
    let f = Fixture::new();
    let font = load_font(f.ctx);
    let white = hg_color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    // 300 ideogramas distintos a 200px (~12 MB de cobertura) não cabem no atlas 2048².
    let huge: String = (0x4E00u32..0x4E00 + 300).filter_map(char::from_u32).collect();
    unsafe {
        assert_eq!(hg_draw_text(f.dl, font, 200.0, 0.0, 0.0, 0.0, white, huge.as_ptr().cast(), huge.len()), OK);
    }
    assert_eq!(f.frame(), [OK; 3]);
}
