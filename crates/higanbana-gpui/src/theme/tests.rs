//! Tema a partir do JSON no formato do gpui-kit (o do desktop).
use super::Theme;
use crate::color::{rgb, rgba};

const DESKTOP: &str = include_str!("../../../higanbana-desktop/themes/higanbana.json");

#[test]
fn parses_desktop_theme() {
    let t = Theme::from_json(DESKTOP).expect("json válido");
    assert!(t.dark);
    assert_eq!(t.primary, rgb(0xe8283c));
    assert_eq!(t.background, rgb(0x07070a));
    assert_eq!(t.border, rgba(0xffffff14));
    assert_eq!(t.popover, rgb(0x111118));
    assert_eq!(t.list_active, rgba(0xe8283c26));
    assert_eq!(t.list_hover, rgba(0xffffff0a));
    assert_eq!(t.success_foreground, rgb(0x86efac));
    assert_eq!(t.destructive_foreground, rgb(0xfca5a5));
    assert_eq!(t.base.red_light, rgb(0xff7a85));
    assert_eq!(t.radius, 8.0);
    assert_eq!(t.radius_lg, 12.0);
    assert_eq!(t.title_bar, rgb(0x07070a));
    assert_eq!(t.title_bar_border, rgba(0xffffff14));
    assert_eq!(t.secondary_hover, rgb(0x1f1f28));
}

#[test]
fn theme_json_missing_keys_default() {
    let t = Theme::from_json(r#"{"themes":[{"mode":"dark","colors":{}}]}"#).unwrap();
    assert_eq!(t, Theme::dark());
    let empty = Theme::from_json("{}").unwrap();
    assert_eq!(empty, Theme::dark());
}

#[test]
fn bad_color_falls_back() {
    let t = Theme::from_json(r##"{"themes":[{"mode":"dark","colors":{"primary.background":"vermelho","ring":"#12"}}]}"##).unwrap();
    assert_eq!(t.primary, Theme::dark().primary);
    assert_eq!(t.ring, Theme::dark().ring);
}

#[test]
fn invalid_json_is_error() {
    assert!(Theme::from_json("{nope").is_err());
}
