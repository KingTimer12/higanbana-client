//! Fio de progresso (linha tracejada com um estame por capítulo) e o realce que
//! mostra o progresso escondido enquanto o mouse mexe.

use std::time::Duration;

use higanbana_core::reader::logic::{strip_progress, Direction};
use higanbana_gpui::prelude::*;

use crate::reader::ReaderUi;
use crate::root::Higanbana;

/// Acima disso o fio vira só linha, sem um estame por capítulo.
const MAX_STAMENS: usize = 120;
/// Quanto o realce dura depois do último movimento.
const STIR: Duration = Duration::from_millis(2500);

pub fn shows_stamens(chapters: usize) -> bool {
    chapters <= MAX_STAMENS
}

impl ReaderUi {
    /// Mouse mexeu: realça e repõe o único timer que apaga o realce.
    /// Devolve se precisa redesenhar (só na entrada do realce).
    pub fn stir(&mut self, cx: &mut EventCx) -> bool {
        if let Some(old) = self.stir_timer.take() {
            cx.cancel_timer(old);
        }
        self.stir_timer = Some(cx.spawn_timer(STIR, |v: &mut Higanbana, cx| {
            if let Some(r) = v.state.reader.as_mut() {
                r.stirred = false;
                r.stir_timer = None;
                cx.notify();
            }
        }));
        !std::mem::replace(&mut self.stirred, true)
    }
}

/// Fio no pé da área da página (`w` = largura da área).
pub fn thread(v: &Higanbana, cx: &mut RenderCx<'_, Higanbana>, w: f32) -> Element<Higanbana> {
    let t = cx.theme().clone();
    let Some(r) = v.state.reader.as_ref() else { return div() };
    let count = r.chapters.len().max(1);
    let current = r.chapter();
    let rtl = r.prefs.direction == Direction::Rtl;
    let fraction = strip_progress(current, r.chapters.len(), r.page(), r.total()) as f32;
    // Linha a 14 px do pé; a barra cheia sai do lado onde a leitura começa.
    let at = |d: Element<Higanbana>, x: f32| if rtl { d.right(x) } else { d.left(x) };
    let mut f = div()
        .absolute()
        .bottom(0.0)
        .left(0.0)
        .right(0.0)
        .h(40.0)
        .child(div().absolute().left(0.0).right(0.0).bottom(14.0).h(1.0).border(1.0, t.border).border_dashed())
        .child(at(div().absolute().bottom(12.8).h(2.0).w(w * fraction).bg(t.primary), 0.0));
    if shows_stamens(r.chapters.len()) {
        let ink = crate::theme::primary_ink(&t);
        for (i, u) in r.chapters.iter().enumerate() {
            let x = w * (i as f32 + 0.5) / count as f32;
            let read = r.progress.get(&u.key).is_some_and(|p| p.read);
            let (h, color) = if i == current {
                (20.0, ink)
            } else if read {
                (12.0, t.primary)
            } else {
                (12.0, t.muted_foreground.fade(0.5))
            };
            let dot = if i == current { 7.2 } else { 4.8 };
            f = f.child(at(div().absolute().bottom(14.0).w(1.0).h(h).bg(color), x)).child(at(div().absolute().bottom(14.0 + h - 3.2).size(dot).rounded_full().bg(color), x - 2.0));
        }
    }
    f
}
