//! Configurações do app num diálogo, aberto pelo botão ao lado do perfil.
//!
//! Por enquanto só o buffer do player. Vale a partir do próximo vídeo aberto.

use std::sync::Arc;

use gpui_kit::{App, Context, FontWeight, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, radio::RadioGroup, switch::Switch, v_flex};
use higanbana_core::{Core, player::store::Settings};

use crate::{
    interact::radios,
    video::{BACK_CACHES, CACHES},
};

pub struct SettingsDialog {
    core: Arc<Core>,
    settings: Settings,
}

impl SettingsDialog {
    pub fn open(core: Arc<Core>, window: &mut Window, cx: &mut App) {
        if window.has_active_dialog(cx) {
            return;
        }
        let settings = core.player().data().settings;
        let view = cx.new(|_| Self { core, settings });
        window.open_dialog(cx, move |dialog, window, _| {
            let view = view.clone();
            dialog.title("Configurações").w(window.rem_size() * 32.).content(move |content, _, _| content.child(view.clone()))
        });
    }

    fn change(&mut self, f: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        f(&mut self.settings);
        // Relê antes de gravar: o player pode ter mudado outros campos.
        let mut saved = self.core.player().data().settings;
        saved.cache_mib = self.settings.cache_mib;
        saved.back_cache_mib = self.settings.back_cache_mib;
        saved.disk_cache = self.settings.disk_cache;
        self.core.player().save_settings(saved);
        cx.notify();
    }
}

fn heading(text: &'static str, cx: &App) -> impl IntoElement {
    div().text_sm().font_weight(FontWeight::SEMIBOLD).child(text).text_color(cx.theme().foreground)
}

fn hint(text: &'static str, cx: &App) -> impl IntoElement {
    div().text_xs().text_color(cx.theme().muted_foreground).child(text)
}

fn mib(n: u32) -> String {
    if n == 0 { "Nada".into() } else { format!("{n} MB") }
}

impl Render for SettingsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.settings.clone();
        v_flex()
            .gap_5()
            .pb_2()
            .child(hint("Vale a partir do próximo vídeo aberto.", cx))
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Buffer à frente", cx))
                    .child(hint("Quanto o player baixa adiantado. Pausado ou cheio, para de baixar.", cx))
                    .child(
                        RadioGroup::horizontal("cfg-ahead")
                            .children(radios(CACHES.map(mib)))
                            .selected_index(CACHES.iter().position(|n| *n == s.cache_mib))
                            .on_change(cx.listener(|v, ix: &usize, _, cx| v.change(|s| s.cache_mib = CACHES[*ix], cx))),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Buffer de volta", cx))
                    .child(hint("Quanto do que já passou fica guardado para voltar sem baixar de novo. O resto é liberado.", cx))
                    .child(
                        RadioGroup::horizontal("cfg-back")
                            .children(radios(BACK_CACHES.map(mib)))
                            .selected_index(BACK_CACHES.iter().position(|n| *n == s.back_cache_mib))
                            .on_change(cx.listener(|v, ix: &usize, _, cx| v.change(|s| s.back_cache_mib = BACK_CACHES[*ix], cx))),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Switch::new("cfg-disk").cursor_pointer().label("Guardar o buffer no disco").checked(s.disk_cache).on_change(cx.listener(
                        |v, on: &bool, _, cx| v.change(|s| s.disk_cache = *on, cx),
                    )))
                    .child(hint("Tira o buffer da RAM, mas grava o vídeo num arquivo temporário enquanto toca.", cx)),
            )
    }
}

