//! Tema a partir do JSON no formato do gpui-kit (`themes[].colors["primary.background"]`...).
//! Chave ausente ou cor inválida mantém o valor do tema padrão do modo.
use super::Theme;
use crate::color::Color;
use serde_json::Value;

/// `#rgb`, `#rrggbb` ou `#rrggbbaa`.
fn parse_hex(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    let n = u32::from_str_radix(h, 16).ok()?;
    let rgba = match h.len() {
        3 => {
            let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
            (r * 17) << 24 | (g * 17) << 16 | (b * 17) << 8 | 0xff
        }
        6 => n << 8 | 0xff,
        8 => n,
        _ => return None,
    };
    Some(crate::color::rgba(rgba))
}

/// Mapa chave do JSON → campo do tema.
fn slot<'t>(t: &'t mut Theme, key: &str) -> Option<&'t mut Color> {
    let b = &mut t.base;
    Some(match key {
        "background" => &mut t.background,
        "foreground" => &mut t.foreground,
        "border" => &mut t.border,
        "input.border" => &mut t.input,
        "ring" => &mut t.ring,
        "selection.background" => &mut t.selection,
        "overlay" => &mut t.overlay,
        "accent.background" => &mut t.accent,
        "accent.foreground" => &mut t.accent_foreground,
        "muted.background" => &mut t.muted,
        "muted.foreground" => &mut t.muted_foreground,
        "primary.background" => &mut t.primary,
        "primary.hover.background" => &mut t.primary_hover,
        "primary.active.background" => &mut t.primary_active,
        "primary.foreground" => &mut t.primary_foreground,
        "secondary.background" => &mut t.secondary,
        "secondary.foreground" => &mut t.secondary_foreground,
        "group_box.background" => &mut t.group_box,
        "popover.background" => &mut t.popover,
        "popover.foreground" => &mut t.popover_foreground,
        "danger.background" => &mut t.destructive,
        "danger.foreground" => &mut t.destructive_foreground,
        "success.background" => &mut t.success,
        "success.foreground" => &mut t.success_foreground,
        "warning.background" => &mut t.warning,
        "warning.foreground" => &mut t.warning_foreground,
        "info.background" => &mut t.info,
        "info.foreground" => &mut t.info_foreground,
        "link.foreground" => &mut t.link,
        "list.hover.background" => &mut t.list_hover,
        "list.active.background" => &mut t.list_active,
        "list.active.border" => &mut t.list_active_border,
        "sidebar.background" => &mut t.sidebar,
        "sidebar.foreground" => &mut t.sidebar_foreground,
        "sidebar.border" => &mut t.sidebar_border,
        "title_bar.background" => &mut t.title_bar,
        "title_bar.border" => &mut t.title_bar_border,
        "secondary.hover.background" => &mut t.secondary_hover,
        "sidebar.accent.background" => &mut t.sidebar_accent,
        "sidebar.accent.foreground" => &mut t.sidebar_accent_foreground,
        "skeleton.background" => &mut t.skeleton,
        "switch.background" => &mut t.switch,
        "scrollbar.thumb.background" => &mut t.scrollbar_thumb,
        "base.red" => &mut b.red,
        "base.red.light" => &mut b.red_light,
        "base.green" => &mut b.green,
        "base.green.light" => &mut b.green_light,
        "base.blue" => &mut b.blue,
        "base.blue.light" => &mut b.blue_light,
        "base.yellow" => &mut b.yellow,
        "base.yellow.light" => &mut b.yellow_light,
        "base.magenta" => &mut b.magenta,
        "base.magenta.light" => &mut b.magenta_light,
        "base.cyan" => &mut b.cyan,
        "base.cyan.light" => &mut b.cyan_light,
        _ => return None,
    })
}

impl Theme {
    /// Primeiro tema do arquivo (o marcado `is_default`, se houver). `card` segue `group_box`.
    pub fn from_json(json: &str) -> Result<Theme, String> {
        let root: Value = serde_json::from_str(json).map_err(|e| format!("tema JSON inválido: {e}"))?;
        let themes = root.get("themes").and_then(Value::as_array);
        let entry = themes.and_then(|a| a.iter().find(|t| t["is_default"] == true).or_else(|| a.first()));
        let Some(entry) = entry else { return Ok(Theme::dark()) };
        let mut t = if entry["mode"] == "light" { Theme::light() } else { Theme::dark() };
        if let Some(r) = entry.get("radius").and_then(Value::as_f64) {
            t.radius = r as f32;
        }
        if let Some(r) = entry.get("radius.lg").and_then(Value::as_f64) {
            t.radius_lg = r as f32;
        }
        let mut card = None;
        if let Some(colors) = entry.get("colors").and_then(Value::as_object) {
            for (k, v) in colors {
                let Some(c) = v.as_str().and_then(parse_hex) else { continue };
                if k == "group_box.background" {
                    card = Some(c);
                }
                if k == "group_box.foreground" {
                    t.card_foreground = c;
                }
                if let Some(dst) = slot(&mut t, k) {
                    *dst = c;
                }
            }
        }
        if let Some(c) = card {
            t.card = c;
        }
        Ok(t)
    }
}
