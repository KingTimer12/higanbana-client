//! Regiões da janela tiradas da árvore (barra customizada).
use crate::gfx::{HitKind, HitRegion};
use crate::layout::tests::build;
use crate::prelude::*;
use crate::Rect;

struct V;

fn regions(t: &crate::tree::Tree<V>) -> Vec<HitRegion> {
    let mut out = Vec::new();
    t.hit_regions(&mut out);
    out
}

#[test]
fn drag_region_and_interactive_children() {
    let bar = div().w(400.0).h(32.0).window_drag().child(div().w(40.0).h(24.0).on_click(|_: &mut V, _, _| {}));
    let r = regions(&build::<V>(div().child(bar), 400.0, 300.0));
    assert_eq!(
        r,
        vec![
            HitRegion { rect: Rect::new(0.0, 0.0, 400.0, 32.0), kind: HitKind::Caption },
            HitRegion { rect: Rect::new(0.0, 0.0, 40.0, 24.0), kind: HitKind::Client },
        ]
    );
}

#[test]
fn maximize_control() {
    let bar = div().w(400.0).h(32.0).window_drag().child(div().w(46.0).h(32.0).window_control(HitKind::Maximize));
    let r = regions(&build::<V>(div().child(bar), 400.0, 300.0));
    assert_eq!(r[1], HitRegion { rect: Rect::new(0.0, 0.0, 46.0, 32.0), kind: HitKind::Maximize });
}

#[test]
fn overlay_over_titlebar_is_client() {
    let bar = div().w(400.0).h(32.0).window_drag().child(div().id("t").w(40.0).h(20.0));
    let root = div().child(bar).child(overlay(Anchor::below("t"), div().w(80.0).h(60.0)).gap(0.0));
    let r = regions(&build::<V>(root, 400.0, 300.0));
    assert_eq!(r.last().unwrap(), &HitRegion { rect: Rect::new(0.0, 20.0, 80.0, 60.0), kind: HitKind::Client });
}

#[test]
fn no_regions_without_marks() {
    assert!(regions(&build::<V>(div().w(10.0).h(10.0).on_click(|_: &mut V, _, _| {}), 100.0, 100.0)).is_empty());
}
