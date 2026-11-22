//! Opções de leitura: modo, emenda, direção, estilo do progresso e atalhos.

use higanbana_core::reader::logic::{Direction, ProgressStyle, ReadMode};
use higanbana_gpui::components::{Button, Component, ComponentSize, Kbd, RadioGroup, Separator, Switch, Variant};
use higanbana_gpui::prelude::*;

use super::{card, heading};
use crate::reader::nav::MODES;
use crate::reader::view::{set_mode, toggle_direction};
use crate::root::Higanbana;

const STYLES: [(ProgressStyle, &str, &str); 3] = [
    (ProgressStyle::Thread, "Fio", "Linha com um estame em cada capítulo."),
    (ProgressStyle::Number, "Só o número", "Número da página na margem, sem linha."),
    (ProgressStyle::Hidden, "Escondido", "Leitura limpa. Aparece ao mover o mouse."),
];

const KEYS: [(&[&str], &str); 7] = [
    (&["left", "right"], "virar a página"),
    (&["[", "]"], "capítulo anterior / próximo"),
    (&["g"], "índice"),
    (&["d"], "trocar direção"),
    (&["m"], "trocar modo"),
    (&["f"], "tela cheia"),
    (&["escape"], "fechar"),
];

fn mode_label(m: ReadMode) -> (&'static str, &'static str) {
    match m {
        ReadMode::Single => ("rectangle-vertical", "Página única"),
        ReadMode::Double => ("columns-2", "Página dupla"),
        ReadMode::Long => ("arrow-down", "Long strip"),
        ReadMode::Wide => ("arrow-right", "Wide strip"),
    }
}

/// Grava as configurações do leitor (valem para todos os títulos).
fn save_settings(v: &mut Higanbana, f: impl FnOnce(&mut higanbana_core::reader::store::ReaderSettings)) {
    if let Some(r) = v.state.reader.as_mut() {
        f(&mut r.settings);
        v.state.core.reader().store.set_settings(r.settings);
    }
}

pub fn panel(v: &mut Higanbana, cx: &mut RenderCx<'_, Higanbana>) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let (mode, seam, progress, rtl) = (r.prefs.mode, r.settings.seam, r.settings.progress, r.prefs.direction == Direction::Rtl);
    let hint = |s: &'static str| div().mt(8.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(text(s));
    let mut modes = div().gap(8.0);
    for m in MODES {
        let (ic, label) = mode_label(m);
        let b = Button::new(("leitor-modo", label), label).variant(Variant::Outline).size(ComponentSize::Compact).icon(icon(ic).size(14.0)).on_click(move |v: &mut Higanbana, _, cx| {
            set_mode(v, m);
            cx.notify();
        });
        let b = b.render(cx);
        modes = modes.child(if m == mode { b.bg(t.accent) } else { b });
    }
    let switch = Switch::new("leitor-emendar", seam).kit().on_change(|v: &mut Higanbana, on, cx| {
        save_settings(v, |s| s.seam = on);
        cx.notify();
    });
    let seam_row = div()
        .mt(20.0)
        .flex_col()
        .gap(4.0)
        .child(div().items_center().gap(8.0).child(switch.render(cx)).child(text("Emendar capítulos").text_size(t.typography.sm)))
        .child(div().pl(28.0).text_size(t.typography.xs).text_color(t.muted_foreground).child(text("Ao terminar, o próximo vem logo em seguida, sem sair do leitor.")));
    let mode_col = div().flex_none().flex_col().child(heading(cx, "Modo")).child(modes).child(seam_row);
    let direction = RadioGroup::new("leitor-direcao-radio", Some(if rtl { 0 } else { 1 })).items(["Direita → esquerda", "Esquerda → direita"]).kit().on_change(move |v: &mut Higanbana, i, cx| {
        if (i == 0) != rtl {
            toggle_direction(v);
        }
        cx.notify();
    });
    let dir_col = div().w(200.0).flex_none().flex_col().child(heading(cx, "Direção")).child(direction.render(cx)).child(hint("Lembrada para este título. Títulos novos começam da direita para a esquerda."));
    let style_ix = STYLES.iter().position(|(p, _, _)| *p == progress).unwrap_or(0);
    let styles = RadioGroup::new("leitor-progresso", Some(style_ix)).items(STYLES.map(|(_, l, _)| l)).kit().on_change(|v: &mut Higanbana, i, cx| {
        save_settings(v, |s| s.progress = STYLES[i].0);
        cx.notify();
    });
    let style_col = div().w(220.0).flex_none().flex_col().child(heading(cx, "Progresso")).child(styles.render(cx)).child(hint(STYLES[style_ix].2));
    let mut keys = div().flex_wrap().gap_x(20.0).gap_y(8.0).text_size(t.typography.xs).text_color(t.muted_foreground);
    for (ks, what) in KEYS {
        let mut k = div().items_center().gap(6.0);
        for key in ks {
            k = k.child(Kbd::new(*key).render(cx));
        }
        keys = keys.child(k.child(text(what)));
    }
    let row = div().items_start().gap(24.0).child(mode_col).child(dir_col).child(style_col);
    // A master corta a coluna Progresso (760 px fixos); aqui o cartão cresce para caber.
    card(cx, "leitor-painel-opcoes").top(16.0).right(16.0).p(24.0).gap(24.0).child(row).child(Separator::horizontal().render(cx)).child(keys)
}
