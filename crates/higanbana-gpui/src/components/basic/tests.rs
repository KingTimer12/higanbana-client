//! Button e Badge sem GPU: clique, desabilitado, teclado, cores e anel de foco.
use super::*;
use crate::anim::Animator;
use crate::components::{ComponentSize, Variant};
use crate::cx::FocusInfo;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::{point, Modifiers, MouseButton, Stats};

#[derive(Default)]
pub(super) struct V {
    pub clicks: u32,
}

/// Roda `f` com um `RenderCx` de teste (tema escuro, foco dado).
pub(super) fn with_cx<R>(anim: &mut Animator, focus: FocusInfo, f: impl FnOnce(&mut RenderCx<'_, V>) -> R) -> R {
    let theme = Theme::dark();
    let mut cx = RenderCx::new(&theme, BackendKind::D3D11, &[], Stats::default(), anim).with_focus(focus);
    f(&mut cx)
}

pub(super) fn render(c: impl Component<V>) -> Element<V> {
    with_cx(&mut Animator::default(), FocusInfo::default(), |cx| c.render(cx))
}

fn button(disabled: bool) -> Element<V> {
    render(Button::new("ok", "OK").disabled(disabled).on_click(|v: &mut V, _, cx| {
        v.clicks += 1;
        cx.notify();
    }))
}

fn click(disabled: bool) -> u32 {
    let mut t = build(div().child(button(disabled)), 200.0, 100.0);
    let (mut v, mut cx) = (V::default(), EventCx::default());
    let m = Modifiers::default();
    t.mouse_down(point(5.0, 5.0), MouseButton::Left, m, &mut v, &mut cx);
    t.mouse_up(point(5.0, 5.0), MouseButton::Left, m, &mut v, &mut cx);
    v.clicks
}

#[test]
fn click_fires_and_disabled_does_not() {
    assert_eq!(click(false), 1);
    assert_eq!(click(true), 0);
    let e = button(true);
    assert_eq!(e.style.opacity, 0.5);
    assert_eq!(e.style.cursor, Some(CursorStyle::NotAllowed));
}

#[test]
fn enter_activates_focused_button() {
    let mut t = build(div().child(button(false)), 200.0, 100.0);
    let (mut v, mut cx) = (V::default(), EventCx::default());
    t.focus("ok");
    assert!(t.key_down(&KeyEvent::new(Key::Enter, Modifiers::default()), &mut v, &mut cx));
    assert_eq!(v.clicks, 1);
}

#[test]
fn button_default_size_and_pointer() {
    let t = build(div().child(button(false)), 200.0, 100.0);
    assert_eq!(t.rect(1).size.height, ComponentSize::Md.height());
    assert_eq!(button(false).style.cursor, Some(CursorStyle::Pointer));
}

#[test]
fn variant_colors_map_to_theme_tokens() {
    let th = Theme::dark();
    for (variant, bg) in [
        (Variant::Default, th.primary),
        (Variant::Secondary, th.secondary),
        (Variant::Destructive, th.destructive),
        (Variant::Outline, th.background),
    ] {
        let e = render(Button::<V>::new("b", "b").variant(variant));
        assert_eq!(e.style.background, Some(bg), "{variant:?}");
    }
    let c = variant_colors(&th, Variant::Default);
    assert_eq!(c.hover_bg, th.primary.alpha(0.9));
    assert_eq!(variant_colors(&th, Variant::Outline).border, th.input);
    let badge = render(Badge::new("x").variant(Variant::Outline));
    assert_eq!(badge.style.border_color, th.border);
    assert_eq!(render(Badge::new("x")).style.background, Some(th.primary));
}

#[test]
fn focus_ring_only_with_keyboard_focus() {
    let mk = |visible| {
        let focus = FocusInfo { focused: Some("ok".into()), visible };
        with_cx(&mut Animator::default(), focus, |cx| Button::<V>::new("ok", "OK").render(cx))
    };
    assert!(mk(false).interact.as_ref().unwrap().focus_style.is_none(), "foco por mouse: sem anel");
    let e = mk(true);
    let ring = (e.interact.as_ref().unwrap().focus_style.as_ref().unwrap())(e.style.clone());
    let th = Theme::dark();
    assert_eq!(ring.shadow.unwrap().color, th.ring.alpha(0.5));
    assert_eq!(ring.border_color, th.ring);
}

#[test]
fn kbd_formata_atalho() {
    use crate::components::Kbd;
    assert_eq!(Kbd::label("ctrl-k"), "Ctrl+K");
    assert_eq!(Kbd::label("alt-left"), "Alt+Left");
    assert_eq!(Kbd::label("shift-enter"), "Shift+Enter");
    assert_eq!(Kbd::label("ctrl-alt-a"), "Ctrl+Alt+A");
    assert_eq!(Kbd::label("shift-pagedown"), "Shift+Page Down");
    assert_eq!(Kbd::label("a"), "A");
}
