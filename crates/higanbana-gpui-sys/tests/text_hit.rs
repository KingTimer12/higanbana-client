//! Posições de cursor e hit test em texto (DirectWrite HitTest*).
use higanbana_gpui_sys::*;
use std::ffi::CString;
mod common;
use common::{Fixture, OK};

fn font(ctx: *mut hg_context) -> hg_font_id {
    let family = CString::new("Segoe UI").unwrap();
    let mut id = 0;
    assert_eq!(unsafe { hg_font_load(ctx, family.as_ptr(), 400.0, 0, &mut id) }, OK);
    id
}

fn caret(ctx: *mut hg_context, f: hg_font_id, s: &str, byte: usize) -> f32 {
    let mut x = -1.0;
    assert_eq!(unsafe { hg_text_caret_x(ctx, f, 16.0, s.as_ptr().cast(), s.len(), byte, &mut x) }, OK);
    x
}

fn hit(ctx: *mut hg_context, f: hg_font_id, s: &str, x: f32) -> usize {
    let mut b = usize::MAX;
    assert_eq!(unsafe { hg_text_hit_test(ctx, f, 16.0, s.as_ptr().cast(), s.len(), x, &mut b) }, OK);
    b
}

#[test]
fn caret_x_grows_and_end_equals_width() {
    let fx = Fixture::new();
    let f = font(fx.ctx);
    let s = "abcd";
    let xs: Vec<f32> = (0..=4).map(|b| caret(fx.ctx, f, s, b)).collect();
    assert_eq!(xs[0], 0.0);
    assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
    let mut m = hg_text_metrics { width: 0.0, height: 0.0, baseline: 0.0 };
    unsafe { hg_text_measure(fx.ctx, f, 16.0, s.as_ptr().cast(), s.len(), 0.0, &mut m) };
    assert!((xs[4] - m.width).abs() < 0.5);
}

#[test]
fn hit_test_round_trips_on_boundaries() {
    let fx = Fixture::new();
    let f = font(fx.ctx);
    for s in ["hello", "日本語テキスト", "a😀b"] {
        let bounds: Vec<usize> = s.char_indices().map(|(i, _)| i).chain([s.len()]).collect();
        for &b in &bounds {
            let x = caret(fx.ctx, f, s, b);
            assert_eq!(hit(fx.ctx, f, s, x + 0.5), b, "{s} @ {b}");
        }
        assert_eq!(hit(fx.ctx, f, s, -50.0), 0);
        assert_eq!(hit(fx.ctx, f, s, 5000.0), s.len());
    }
}

#[test]
fn batch_positions_match_single_calls() {
    let fx = Fixture::new();
    let f = font(fx.ctx);
    let s = "a😀日b";
    let n = s.chars().count() + 1;
    let mut xs = vec![0.0f32; n];
    assert_eq!(unsafe { hg_text_caret_positions(fx.ctx, f, 16.0, s.as_ptr().cast(), s.len(), xs.as_mut_ptr(), n) }, OK);
    for (k, b) in s.char_indices().map(|(i, _)| i).chain([s.len()]).enumerate() {
        assert!((xs[k] - caret(fx.ctx, f, s, b)).abs() < 0.01);
    }
    let bad = unsafe { hg_text_caret_positions(fx.ctx, f, 16.0, s.as_ptr().cast(), s.len(), xs.as_mut_ptr(), n - 1) };
    assert_eq!(bad, HG_ERR_INVALID as i32);
}

#[test]
fn empty_text_caret_is_zero() {
    let fx = Fixture::new();
    let f = font(fx.ctx);
    assert_eq!(caret(fx.ctx, f, "", 0), 0.0);
    assert_eq!(hit(fx.ctx, f, "", 10.0), 0);
}
