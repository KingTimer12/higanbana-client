//! Movimento do produto: só para explicar uma mudança (algo entrou, saiu,
//! trocou de lugar), curto e sempre com saída para "reduzir movimento".
//!
//! `with_animation` do GPUI já pula para o quadro final quando
//! `App::reduce_motion` está ligado; aqui lemos essa preferência do sistema.

use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt as _, App, Hsla, IntoElement, ParentElement, Styled, div, pulsating_between, px, rems,
};

/// Entrada de superfícies pequenas (painéis, cartões, imagens).
pub const QUICK: Duration = Duration::from_millis(160);
/// Troca de tela e barras que entram na janela.
pub const BASE: Duration = Duration::from_millis(240);
/// Entrada em cascata de uma lista de cards: cada um sobe `RISE_DISTANCE` px e
/// aparece em `RISE`, começando `STAGGER_STEP` depois do anterior. A espera para
/// de crescer em `STAGGER_CAP` itens, para o fim de uma lista longa não demorar.
pub const RISE: Duration = Duration::from_millis(420);
pub const RISE_DISTANCE: f32 = 14.;
pub const STAGGER_STEP: Duration = Duration::from_millis(40);
pub const STAGGER_CAP: usize = 12;

/// Desacelera no fim: o movimento chega devagar ao lugar.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}

/// Desacelera passando um pouco do lugar e volta: algo que "desabrocha".
/// Passa de 1 por volta de 5 % no meio do caminho e termina exatamente em 1.
pub fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.2;
    const C3: f32 = C1 + 1.;
    let u = t.clamp(0., 1.) - 1.;
    1. + C3 * u.powi(3) + C1 * u.powi(2)
}

pub fn enter(duration: Duration) -> Animation {
    Animation::new(duration).with_easing(ease_out_cubic)
}

/// Quanto o item `index` espera antes de começar a subir.
pub fn stagger_delay(index: usize) -> Duration {
    STAGGER_STEP * index.min(STAGGER_CAP) as u32
}

/// Progresso (já com a curva) da entrada de um item que espera `delay` e sobe
/// em `RISE`, `elapsed` depois do começo da lista.
pub fn rise_progress(elapsed: Duration, delay: Duration) -> f32 {
    ease_out_cubic(elapsed.saturating_sub(delay).as_secs_f32() / RISE.as_secs_f32())
}

/// Animação de entrada do item `index` numa cascata (ver [`RISE`]): o valor
/// fica em 0 durante a espera e chega a 1 com a curva de saída.
pub fn staggered(index: usize) -> Animation {
    let delay = stagger_delay(index);
    let total = delay + RISE;
    Animation::new(total).with_easing(move |t| rise_progress(total.mul_f32(t.clamp(0., 1.)), delay))
}

/// Equalizador da faixa que está tocando: três barras em ritmos diferentes,
/// para não pulsarem juntas. Só aparece enquanto toca de verdade.
pub fn playing_bars(color: Hsla) -> impl IntoElement {
    div().flex().items_end().gap(px(2.)).h(rems(1.)).children([0u64, 1, 2].map(|i| {
        div()
            .w(px(3.))
            .h(rems(1.))
            .rounded_full()
            .bg(color)
            .with_animation(
                ("playing-bar", i as usize),
                Animation::new(Duration::from_millis(760 + i * 170)).repeat().with_easing(pulsating_between(0.3, 1.)),
                |d, t| d.h(rems(t)),
            )
    }))
}

/// Segue "Mostrar animações no Windows" (Acessibilidade › Efeitos visuais).
pub fn sync_system_preference(cx: &mut App) {
    cx.set_reduce_motion(!system_animations_enabled());
}

#[cfg(windows)]
fn system_animations_enabled() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};
    let mut on: i32 = 1;
    // SAFETY: o ponteiro aponta para um BOOL vivo durante a chamada, como a API pede.
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut on as *mut i32).cast(), 0) };
    ok == 0 || on != 0
}

#[cfg(not(windows))]
fn system_animations_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_starts_and_ends_in_place() {
        assert_eq!(ease_out_cubic(0.), 0.);
        assert_eq!(ease_out_cubic(1.), 1.);
        assert!(ease_out_cubic(0.5) > 0.5, "desacelera: passa da metade na metade do tempo");
        assert_eq!(ease_out_cubic(2.), 1.);
    }

    #[test]
    fn stagger_spaces_items_and_stops_growing() {
        assert_eq!(stagger_delay(0), Duration::ZERO);
        assert_eq!(stagger_delay(1), Duration::from_millis(40));
        assert_eq!(stagger_delay(3), Duration::from_millis(120));
        assert_eq!(stagger_delay(STAGGER_CAP), stagger_delay(500), "lista longa não espera mais que o teto");
        assert!(stagger_delay(500) <= Duration::from_millis(500));
    }

    #[test]
    fn rise_waits_its_turn_then_finishes() {
        let delay = stagger_delay(2);
        assert_eq!(rise_progress(Duration::ZERO, delay), 0.);
        assert_eq!(rise_progress(delay, delay), 0., "parado durante a espera");
        let mid = rise_progress(delay + RISE / 2, delay);
        assert!(mid > 0.5 && mid < 1., "desacelera: {mid}");
        assert_eq!(rise_progress(delay + RISE, delay), 1.);
        assert_eq!(rise_progress(delay + RISE * 3, delay), 1.);
    }

    #[test]
    fn staggered_animation_covers_wait_and_rise() {
        let a = staggered(3);
        assert_eq!(a.duration, stagger_delay(3) + RISE);
        assert_eq!((a.easing)(0.), 0.);
        assert_eq!((a.easing)(1.), 1.);
        assert_eq!((a.easing)(0.1), 0., "ainda esperando a vez");
    }

    #[test]
    fn ease_out_back_overshoots_then_settles() {
        assert!(ease_out_back(0.).abs() < 1e-6);
        assert!((ease_out_back(1.) - 1.).abs() < 1e-6);
        let peak = (1..100).map(|i| ease_out_back(i as f32 / 100.)).fold(0., f32::max);
        assert!(peak > 1. && peak < 1.1, "passa um pouco do lugar: {peak}");
    }
}
