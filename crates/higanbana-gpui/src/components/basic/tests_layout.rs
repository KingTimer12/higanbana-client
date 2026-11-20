//! Card, Separator, Skeleton e Avatar sem GPU.
use super::tests::{render, with_cx};
use crate::components::ComponentSize;
use super::*;
use crate::anim::Animator;
use crate::cx::FocusInfo;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::ImageData;

#[test]
fn card_slots_stack_with_padding() {
    let card = render(Card::new().header(CardHeader::new("TÃ­tulo").description("Desc")).content(div().h(40.0)).footer(div().h(20.0)));
    let t = build(div().flex_col().items_start().child(card), 400.0, 400.0);
    // 0 raiz, 1 card, 2 header(3 tÃ­tulo div, 4 texto, 5 desc), 6 content(7), 8 footer(9)
    let (h, c, f) = (t.rect(2), t.rect(6), t.rect(8));
    assert_eq!(h.origin.y, 1.0 + 24.0, "borda + py-6");
    assert_eq!(c.origin.y, h.origin.y + h.size.height + 24.0, "gap-6");
    assert_eq!(f.origin.y, c.origin.y + 40.0 + 24.0);
    assert_eq!(t.rect(7).origin.x, 1.0 + 24.0, "px-6");
    assert_eq!(t.rect(5).origin.y, t.rect(3).origin.y + 16.0 + 6.0, "descriÃ§Ã£o abaixo do tÃ­tulo");
}

#[test]
fn separator_is_one_pixel() {
    let root = div()
        .flex_col()
        .w(100.0)
        .child(render(Separator::horizontal()))
        .child(div().h(30.0).child(render(Separator::vertical())));
    let t = build(root, 200.0, 200.0);
    assert_eq!(t.rect(1).size, crate::size(100.0, 1.0));
    assert_eq!(t.rect(3).size, crate::size(1.0, 30.0));
    assert_eq!(render(Separator::horizontal()).style.background, Some(Theme::dark().border));
}

#[test]
fn skeleton_animates_then_idles_when_removed() {
    let mut anim = Animator::default();
    let frame = |show: bool, anim: &mut Animator| {
        anim.begin();
        let wake = with_cx(anim, FocusInfo::default(), |cx| {
            if show {
                let _ = Skeleton::new("sk").w(50.0).h(10.0).render(cx);
            }
            cx.wake
        });
        anim.end();
        wake
    };
    assert!(frame(true, &mut anim).is_some(), "pede frame enquanto presente");
    assert_eq!(anim.len(), 1);
    assert!(frame(false, &mut anim).is_none(), "sem skeleton: sem frames");
    assert!(anim.is_empty(), "tween descartado");
}

#[test]
fn skeleton_uses_muted_and_pulses_in_range() {
    let e = render(Skeleton::new("sk").w(10.0).h(10.0));
    assert_eq!(e.style.background, Some(Theme::dark().muted));
    assert!((0.5..=1.0).contains(&e.style.opacity));
}

#[test]
fn avatar_initials_fallback_and_image() {
    assert_eq!(initials("ana beatriz de souza"), "AS");
    assert_eq!(initials("Kai"), "K");
    assert_eq!(initials("  "), "");
    let e = render(Avatar::new("Ana Beatriz"));
    assert!(matches!(&e.children[0].kind, crate::element::Kind::Text(s) if s == "AB"));
    assert_eq!(e.style.width, px(32.0));
    let src = ImageData::from_rgba8(1, 1, vec![0; 4]);
    let e = render(Avatar::new("Ana").image(src).size(ComponentSize::Lg));
    assert!(matches!(e.children[0].kind, crate::element::Kind::Image(_)));
    assert_eq!(e.style.width, px(40.0));
}
