//! Kbd (kit): atalho de teclado como "Ctrl+K", fundo muted e texto xs.
use super::Component;
use crate::cx::RenderCx;
use crate::element::{div, text, Element};
use crate::style::Styled;

/// `Kbd::new("ctrl-k")`: teclas separadas por `-`, modificadores antes.
pub struct Kbd {
    stroke: String,
    outline: bool,
}

impl Kbd {
    pub fn new(stroke: impl Into<String>) -> Self {
        Kbd { stroke: stroke.into(), outline: false }
    }

    /// Com borda e fundo do tema (variante `outline` do kit).
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// Texto exibido no Windows/Linux, como o kit: `ctrl-k` → `Ctrl+K`.
    pub fn label(stroke: &str) -> String {
        let mut mods = Vec::new();
        let mut key = String::new();
        for part in stroke.split('-').filter(|p| !p.is_empty()) {
            match part {
                "ctrl" => mods.push("Ctrl"),
                "alt" => mods.push("Alt"),
                "shift" => mods.push("Shift"),
                "cmd" | "win" | "super" => mods.push("Win"),
                k => key = key_name(k),
            }
        }
        let mut parts: Vec<String> = mods.into_iter().map(str::to_owned).collect();
        parts.push(key);
        parts.join("+")
    }
}

fn key_name(k: &str) -> String {
    match k {
        "pageup" => "Page Up".into(),
        "pagedown" => "Page Down".into(),
        "escape" => "Esc".into(),
        _ if k.chars().count() == 1 => k.to_uppercase(),
        _ => {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    }
}

impl<V> Component<V> for Kbd {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = cx.theme();
        let e = div()
            .flex_shrink_0()
            .justify_center()
            .px(4.0)
            .py(2.0)
            .min_w(20.0)
            .rounded(t.radius / 2.0)
            .text_color(t.muted_foreground)
            .text_size(t.typography.xs)
            .whitespace_nowrap()
            .child(text(Self::label(&self.stroke)));
        if self.outline {
            e.border(1.0, t.border).bg(t.background)
        } else {
            e.bg(t.muted)
        }
    }
}
