//! Paletas claro/escuro (valores do tema "zinc" do shadcn, com primária vermelho-higanbana).
use super::{Spacing, Theme, Typography};
use crate::color::rgb;

pub(super) fn light() -> Theme {
    Theme {
        dark: false,
        background: rgb(0xffffff),
        foreground: rgb(0x09090b),
        card: rgb(0xffffff),
        card_foreground: rgb(0x09090b),
        popover: rgb(0xffffff),
        popover_foreground: rgb(0x09090b),
        primary: rgb(0xdc2626),
        primary_foreground: rgb(0xfef2f2),
        secondary: rgb(0xf4f4f5),
        secondary_foreground: rgb(0x18181b),
        muted: rgb(0xf4f4f5),
        muted_foreground: rgb(0x71717a),
        accent: rgb(0xf4f4f5),
        accent_foreground: rgb(0x18181b),
        destructive: rgb(0xef4444),
        destructive_foreground: rgb(0xfafafa),
        border: rgb(0xe4e4e7),
        input: rgb(0xe4e4e7),
        ring: rgb(0xdc2626),
        radius: 8.0,
        spacing: Spacing::default(),
        typography: Typography::default(),
    }
}

pub(super) fn dark() -> Theme {
    Theme {
        dark: true,
        background: rgb(0x09090b),
        foreground: rgb(0xfafafa),
        card: rgb(0x18181b),
        card_foreground: rgb(0xfafafa),
        popover: rgb(0x09090b),
        popover_foreground: rgb(0xfafafa),
        primary: rgb(0xdc2626),
        primary_foreground: rgb(0xfef2f2),
        secondary: rgb(0x27272a),
        secondary_foreground: rgb(0xfafafa),
        muted: rgb(0x27272a),
        muted_foreground: rgb(0xa1a1aa),
        accent: rgb(0x27272a),
        accent_foreground: rgb(0xfafafa),
        destructive: rgb(0x7f1d1d),
        destructive_foreground: rgb(0xfafafa),
        border: rgb(0x27272a),
        input: rgb(0x27272a),
        ring: rgb(0xdc2626),
        radius: 8.0,
        spacing: Spacing::default(),
        typography: Typography::default(),
    }
}
