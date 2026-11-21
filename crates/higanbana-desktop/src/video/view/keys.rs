//! Atalhos do player (ignorados com Ctrl/Alt).

use std::time::Instant;

use higanbana_gpui::prelude::*;

use crate::root::Higanbana;

/// O que uma tecla faz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyAction {
    TogglePause,
    Seek(f64),
    Volume(f64),
    Mute,
    Fullscreen,
    Next,
    SkipIntro,
    /// Esc: fecha menu → sai da tela cheia → fecha o player.
    Escape,
}

pub fn action(ev: &KeyEvent) -> Option<KeyAction> {
    if ev.mods.ctrl || ev.mods.alt {
        return None;
    }
    Some(match ev.key {
        Key::Space => KeyAction::TogglePause,
        Key::Left => KeyAction::Seek(-5.0),
        Key::Right => KeyAction::Seek(5.0),
        Key::Up => KeyAction::Volume(5.0),
        Key::Down => KeyAction::Volume(-5.0),
        Key::Char('m') | Key::Char('M') => KeyAction::Mute,
        Key::Char('f') | Key::Char('F') => KeyAction::Fullscreen,
        Key::Char('n') | Key::Char('N') => KeyAction::Next,
        Key::Char('s') | Key::Char('S') => KeyAction::SkipIntro,
        Key::Escape => KeyAction::Escape,
        _ => return None,
    })
}

/// Esc em camadas.
#[derive(Debug, PartialEq)]
pub enum EscapeStep {
    CloseMenu,
    ExitFullscreen,
    ClosePlayer,
}

pub fn escape_step(menu_open: bool, fullscreen: bool) -> EscapeStep {
    if menu_open {
        EscapeStep::CloseMenu
    } else if fullscreen {
        EscapeStep::ExitFullscreen
    } else {
        EscapeStep::ClosePlayer
    }
}

/// Volume novo (0..100) e se tira do mudo.
pub fn volume_step(volume: f64, muted: bool, delta: f64) -> (f64, bool) {
    let v = (volume + delta).clamp(0.0, 100.0);
    (v, muted && v > 0.0)
}

pub fn on_key(v: &mut Higanbana, ev: &KeyEvent, cx: &mut EventCx) {
    let Some(a) = action(ev) else { return };
    let now = Instant::now();
    let fullscreen = v.window_fullscreen;
    let Some(p) = v.player.as_mut() else { return };
    match a {
        KeyAction::TogglePause => p.player.toggle_pause(),
        KeyAction::Seek(d) => {
            let t = (p.session.pos() + d).min(p.session.dur() - 0.5).max(0.0);
            p.player.seek(t);
        }
        KeyAction::Volume(d) => {
            let (vol, unmute) = volume_step(p.last.volume, p.last.muted, d);
            p.player.set_volume(vol);
            if unmute {
                p.player.set_property("mute", "no");
            }
        }
        KeyAction::Mute => p.player.toggle_mute(),
        KeyAction::Fullscreen => cx.window_command(WindowCommand::ToggleFullscreen),
        KeyAction::Next => {
            let acts = p.session.go_next(now);
            p.run(acts);
        }
        KeyAction::SkipIntro => {
            if let Some(to) = p.session.skip_intro() {
                p.player.seek(to);
            }
        }
        KeyAction::Escape => match escape_step(p.menu.is_some(), fullscreen) {
            EscapeStep::CloseMenu => {
                p.menu = None;
                p.pops = Default::default();
            }
            EscapeStep::ExitFullscreen => cx.window_command(WindowCommand::ExitFullscreen),
            EscapeStep::ClosePlayer => {
                cx.stop_propagation();
                v.close_player(cx);
                return;
            }
        },
    }
    p.poke(now);
    cx.stop_propagation();
    cx.notify();
}
