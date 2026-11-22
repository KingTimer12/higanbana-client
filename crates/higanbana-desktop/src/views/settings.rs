//! Configurações do app num diálogo, aberto pelo botão ao lado do perfil.
//! Por enquanto só o buffer do player; vale a partir do próximo vídeo aberto.

use higanbana_core::player::store::Settings;
use higanbana_gpui::components::{Component, Dialog, RadioGroup, Switch};
use higanbana_gpui::prelude::*;

use crate::root::Higanbana;
use crate::video::{BACK_CACHES, CACHES};

pub fn mib(n: u32) -> String {
    if n == 0 { "Nada".into() } else { format!("{n} MB") }
}

/// Os campos deste diálogo por cima do que está gravado (o player pode ter mudado outros).
pub fn merge(mut saved: Settings, edited: &Settings) -> Settings {
    saved.cache_mib = edited.cache_mib;
    saved.back_cache_mib = edited.back_cache_mib;
    saved.disk_cache = edited.disk_cache;
    saved
}

/// Relê do núcleo, aplica `f` e grava.
fn change(v: &mut Higanbana, f: impl FnOnce(&mut Settings)) {
    let player = v.state.core.player();
    let mut edited = player.data().settings;
    f(&mut edited);
    player.save_settings(merge(player.data().settings, &edited));
}

pub fn dialog(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let s = v.state.core.player().data().settings;
    let heading = |s: &'static str| div().text_size(t.typography.sm).font_weight(600).text_color(t.foreground).child(text(s));
    let hint = |s: &'static str| div().text_size(t.typography.xs).text_color(t.muted_foreground).child(text(s));
    let ahead = RadioGroup::new("cfg-frente", CACHES.iter().position(|n| *n == s.cache_mib))
        .items(CACHES.map(mib))
        .horizontal()
        .kit()
        .on_change(|v: &mut Higanbana, i, cx| {
            change(v, |s| s.cache_mib = CACHES[i]);
            cx.notify();
        })
        .render(cx);
    let back = RadioGroup::new("cfg-volta", BACK_CACHES.iter().position(|n| *n == s.back_cache_mib))
        .items(BACK_CACHES.map(mib))
        .horizontal()
        .kit()
        .on_change(|v: &mut Higanbana, i, cx| {
            change(v, |s| s.back_cache_mib = BACK_CACHES[i]);
            cx.notify();
        })
        .render(cx);
    let disk = Switch::new("cfg-disco", s.disk_cache)
        .kit()
        .on_change(|v: &mut Higanbana, on, cx| {
            change(v, |s| s.disk_cache = on);
            cx.notify();
        })
        .render(cx);
    let body = div()
        .flex_col()
        .gap(16.0)
        .pb(8.0)
        .child(hint("Vale a partir do próximo vídeo aberto."))
        .child(div().flex_col().gap(8.0).child(heading("Buffer à frente")).child(hint("Quanto o player baixa adiantado. Pausado ou cheio, para de baixar.")).child(ahead))
        .child(div().flex_col().gap(8.0).child(heading("Buffer de volta")).child(hint("Quanto do que já passou fica guardado para voltar sem baixar de novo. O resto é liberado.")).child(back))
        .child(
            div()
                .flex_col()
                .gap(8.0)
                .child(div().items_center().gap(8.0).child(disk).child(text("Guardar o buffer no disco").text_size(t.typography.sm)))
                .child(hint("Tira o buffer da RAM, mas grava o vídeo num arquivo temporário enquanto toca.")),
        );
    Dialog::new("configuracoes", v.settings_open)
        .title("Configurações")
        .kit()
        .width(512.0)
        .child(body)
        .on_open_change(|v: &mut Higanbana, open, cx| {
            v.settings_open = open;
            cx.notify();
        })
        .render(cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mib_zero_e_nada() {
        assert_eq!(mib(0), "Nada");
        assert_eq!(mib(150), "150 MB");
    }

    #[test]
    fn salvar_nao_apaga_outros_campos() {
        let saved = Settings { auto_next: false, sub_lang: "pt".into(), cache_mib: 50, ..Settings::default() };
        let edited = Settings { cache_mib: 300, disk_cache: true, auto_next: true, ..Settings::default() };
        let out = merge(saved, &edited);
        assert_eq!((out.cache_mib, out.disk_cache), (300, true));
        assert_eq!((out.auto_next, out.sub_lang.as_str()), (false, "pt"), "campos do player ficam");
    }
}
