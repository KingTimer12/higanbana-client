//! Progress (trilho + preenchimento) e Spinner (ícone girando só enquanto renderizado).
use super::tests::{with_cx, V};
use super::{Progress, ProgressSize, Spinner};
use crate::anim::Animator;
use crate::components::Component;
use crate::cx::FocusInfo;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::style::Styled;

fn bar(value: f32) -> crate::Tree<V> {
    let e = with_cx(&mut Animator::default(), FocusInfo::default(), |cx| Progress::new("p").value(value).render(cx));
    build(div().w(200.0).child(e), 300.0, 100.0)
}

fn fill_width(t: &crate::Tree<V>) -> f32 {
    t.rect(t.find(Progress::<V>::fill_id("p")).unwrap()).size.width
}

#[test]
fn progress_fill_width() {
    let t = bar(25.0);
    assert_eq!(t.rect(1).size.width, 200.0);
    assert_eq!(fill_width(&t), 50.0);
}

#[test]
fn progress_clamps() {
    assert_eq!(fill_width(&bar(-5.0)), 0.0);
    assert_eq!(fill_width(&bar(150.0)), 200.0);
}

#[test]
fn progress_sizes_match_kit() {
    for (s, h) in [(ProgressSize::XSmall, 4.0), (ProgressSize::Small, 6.0), (ProgressSize::Medium, 8.0), (ProgressSize::Large, 10.0)] {
        let e = with_cx(&mut Animator::default(), FocusInfo::default(), |cx| Progress::new("p").size(s).render(cx));
        let t = build(div().w(100.0).child(e), 300.0, 100.0);
        assert_eq!(t.rect(1).size.height, h);
    }
}

#[test]
fn spinner_requests_frames_only_when_rendered() {
    let mut anim = Animator::default();
    anim.begin();
    with_cx(&mut anim, FocusInfo::default(), |cx| Spinner::new("s").render(cx));
    assert!(anim.is_active(), "girando agenda frames");
    anim.end();
    anim.begin();
    anim.end();
    anim.begin();
    assert!(!anim.is_active(), "fora da árvore para de agendar");
}

#[test]
fn rotated_icon_differs() {
    let a = crate::icon::raster::get_rotated("loader", 24, 0).unwrap();
    let b = crate::icon::raster::get_rotated("loader", 24, 5).unwrap();
    assert_ne!(a.id(), b.id());
    let px = |s: &ImageSource| (0..24).flat_map(|y| (0..24).map(move |x| (x, y))).map(|(x, y)| s.pending_pixel(x, y).unwrap()[3] as u32).collect::<Vec<_>>();
    assert_ne!(px(&a), px(&b));
}
