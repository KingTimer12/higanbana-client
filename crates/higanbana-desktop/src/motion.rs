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
    fn ease_out_back_overshoots_then_settles() {
        assert!(ease_out_back(0.).abs() < 1e-6);
        assert!((ease_out_back(1.) - 1.).abs() < 1e-6);
        let peak = (1..100).map(|i| ease_out_back(i as f32 / 100.)).fold(0., f32::max);
        assert!(peak > 1. && peak < 1.1, "passa um pouco do lugar: {peak}");
    }
}
