//! Pedidos dos handlers (foco, timers, frames) e o relógio do laço.
use super::run::{Dirty, Runner};
use super::View;
use crate::cx::EventCx;
use crate::gfx::{Shader, WindowState};
use std::time::{Duration, Instant};

/// Intervalo de frame quando não há vsync para frear o laço.
const FRAME: Duration = Duration::from_millis(16);
/// Intervalo enquanto o present está ocluído: nada aparece, então basta perceber a volta
/// (~10 quadros/s, imperceptível ao desbloquear) gastando o mínimo de CPU.
const OCCLUDED_FRAME: Duration = Duration::from_millis(100);

/// Espera até o próximo quadro pedido. Com vsync o `present` freia o ritmo (zero), mas não
/// quando foi ocluído: o Present volta na hora e o laço giraria um núcleo inteiro.
pub(super) fn next_frame_delay(vsync: bool, occluded: bool) -> Duration {
    match (vsync, occluded) {
        (_, true) => OCCLUDED_FRAME,
        (true, false) => Duration::ZERO,
        (false, false) => FRAME,
    }
}

impl<V: View> Runner<'_, V> {
    /// Executa os pedidos feitos pelos handlers. `false` = encerrar.
    pub(super) fn apply(&mut self, mut cx: EventCx) -> bool {
        if cx.quit {
            return false;
        }
        let now = Instant::now();
        if let Some(theme) = cx.theme.take() {
            self.theme = theme;
        }
        if let Some(kind) = cx.switch_to {
            match self.ctx.switch_backend(kind) {
                Err(e) => eprintln!("higanbana-gpui: troca para {} falhou: {e}", kind.label()),
                Ok(()) if !self.shaders.is_empty() => {
                    // Shader que falhou no backend anterior tenta de novo neste (sai do magenta).
                    self.shaders.reset_errors();
                    self.mark(Dirty::Repaint);
                }
                Ok(()) => {}
            }
        }
        for c in cx.window_commands.drain(..) {
            self.win.command(c);
        }
        if let Some(req) = cx.focus {
            self.tree.apply_focus(req);
        }
        self.timers.absorb(&mut cx, now);
        self.tasks.absorb(&mut cx, &self.win.waker());
        if !cx.scroll_requests.is_empty() {
            self.scrolls.append(&mut cx.scroll_requests);
            self.mark(Dirty::Repaint);
        }
        if cx.animation_frame {
            self.request_frame(now);
        }
        if cx.notified {
            self.mark(Dirty::Rebuild);
        }
        true
    }

    /// Reconstrói no próximo frame (com vsync o `present` já freia o ritmo).
    pub(super) fn request_frame(&mut self, now: Instant) {
        let at = now + next_frame_delay(self.vsync, self.occluded);
        self.wake = Some(self.wake.map_or(at, |w| w.min(at)));
    }

    /// Próximo instante em que o laço precisa acordar sem evento (`None` = dormir).
    pub(super) fn deadline(&self) -> Option<Instant> {
        [self.timers.next_deadline(), self.wake, self.blink, self.repaint].into_iter().flatten().min()
    }

    /// Depois do `present`: agenda o próximo quadro de shader animado, cria os shaders
    /// que faltaram no cache e passa adiante erros do compilador nativo (magenta).
    pub(super) fn after_shaders(&mut self, animated: bool, missed: Vec<Shader>, now: Instant) {
        self.repaint = shader_repaint_at(animated, self.window_state, self.vsync, self.occluded, now);
        let mut fresh = false;
        for s in &missed {
            fresh |= self.shaders.add(s);
        }
        if fresh || (!self.shaders.is_empty() && self.shaders.collect_errors()) {
            self.mark(Dirty::Repaint);
        }
    }

    /// Roda o que venceu: timers, reconstrução pedida e piscar do cursor. `false` = encerrar.
    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if let Some(cx) = self.inbox.as_mut().and_then(|i| i.drain(&mut self.view))
            && !self.apply(cx) {
                return false;
            }
        if let Some(cx) = self.tasks.drain(&mut self.view)
            && !self.apply(cx) {
                return false;
            }
        if let Some(cx) = self.timers.fire_due(now, &mut self.view)
            && !self.apply(cx) {
                return false;
            }
        if self.wake.is_some_and(|w| w <= now) {
            self.wake = None;
            self.mark(Dirty::Rebuild);
        }
        if self.repaint.is_some_and(|r| r <= now) {
            self.repaint = None;
            self.mark(Dirty::Repaint);
        }
        if self.blink.is_some_and(|b| b <= now) {
            self.blink = None;
            self.mark(Dirty::Repaint);
        }
        true
    }
}

/// Quando repintar por causa de shader animado: só se foi pintado neste quadro e a janela
/// não está minimizada. Sem foco ou coberta continua animando (como o vídeo); com o present
/// ocluído, devagar (ver `next_frame_delay`).
pub(super) fn shader_repaint_at(
    painted_animated: bool,
    state: WindowState,
    vsync: bool,
    occluded: bool,
    now: Instant,
) -> Option<Instant> {
    (painted_animated && !state.minimized).then(|| now + next_frame_delay(vsync, occluded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::WindowState;

    #[test]
    fn shader_animado_pintado_pede_quadro_imediato() {
        let now = Instant::now();
        assert_eq!(shader_repaint_at(true, WindowState::default(), true, false, now), Some(now));
        assert_eq!(shader_repaint_at(true, WindowState::default(), false, false, now), Some(now + FRAME));
    }

    #[test]
    fn present_ocluido_nao_gira_o_laco() {
        // Tela bloqueada/monitor desligado: o Present volta na hora (DXGI_STATUS_OCCLUDED) e o
        // vsync não freia mais; o próximo quadro espera OCCLUDED_FRAME em vez de `now`.
        let now = Instant::now();
        assert_eq!(next_frame_delay(true, false), Duration::ZERO);
        assert_eq!(next_frame_delay(false, false), FRAME);
        assert_eq!(next_frame_delay(true, true), OCCLUDED_FRAME);
        assert_eq!(next_frame_delay(false, true), OCCLUDED_FRAME);
        assert!(OCCLUDED_FRAME >= FRAME);
        let w = WindowState::default();
        assert_eq!(shader_repaint_at(true, w, true, true, now), Some(now + OCCLUDED_FRAME));
        assert_eq!(shader_repaint_at(true, w, true, false, now), Some(now));
    }

    #[test]
    fn shader_animado_sem_prazo_se_minimizada_ou_nao_pintado() {
        let now = Instant::now();
        let min = WindowState { minimized: true, ..Default::default() };
        assert_eq!(shader_repaint_at(true, min, true, false, now), None);
        assert_eq!(shader_repaint_at(false, WindowState::default(), true, false, now), None);
        // Sem foco continua animando (como o vídeo).
        let unfocused = WindowState { focused: false, ..Default::default() };
        assert_eq!(shader_repaint_at(true, unfocused, true, false, now), Some(now));
    }
}
