//! Contexto de handlers: marca a view suja e enfileira pedidos ao app.
use super::task::TaskId;
use super::timer::{TimerFn, TimerId};
use crate::element::ElementId;
use crate::gfx::BackendKind;
use crate::theme::Theme;
use std::any::Any;
use std::time::Duration;

/// Pedido de foco feito por um handler (aplicado depois dele).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusRequest {
    Focus(ElementId),
    Blur,
    Next,
    Prev,
}

/// Contexto entregue a todo handler (`on_click`, `on_key_down`, timers...).
#[derive(Default)]
pub struct EventCx {
    pub(crate) notified: bool,
    pub(crate) switch_to: Option<BackendKind>,
    pub(crate) theme: Option<Theme>,
    pub(crate) quit: bool,
    pub(crate) stopped: bool,
    pub(crate) focus: Option<FocusRequest>,
    pub(crate) animation_frame: bool,
    pub(crate) timers: Vec<(Duration, TimerId, TimerFn)>,
    pub(crate) cancelled: Vec<TimerId>,
    pub(crate) window_commands: Vec<crate::gfx::WindowCommand>,
    pub(crate) tasks: Vec<super::task::TaskReq>,
    pub(crate) cancelled_tasks: Vec<super::task::TaskId>,
}

impl EventCx {
    /// O estado mudou: reconstrói a árvore no próximo frame.
    pub fn notify(&mut self) {
        self.notified = true;
    }

    pub fn is_notified(&self) -> bool {
        self.notified
    }

    /// Minimizar, maximizar, fechar, fullscreen (aplicado depois do handler).
    pub fn window_command(&mut self, c: crate::gfx::WindowCommand) {
        self.window_commands.push(c);
    }

    /// Para a propagação de teclas/caracteres para os ancestrais.
    pub fn stop_propagation(&mut self) {
        self.stopped = true;
    }

    pub fn propagation_stopped(&self) -> bool {
        self.stopped
    }

    /// Troca o backend de GPU depois do handler (sem fechar a janela).
    pub fn switch_backend(&mut self, kind: BackendKind) {
        self.switch_to = Some(kind);
        self.notified = true;
    }

    /// Substitui o tema do app.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
        self.notified = true;
    }

    /// Fecha a janela e encerra `App::run`.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// Move o foco para o elemento `id` (precisa ser `.focusable()`).
    pub fn focus(&mut self, id: impl Into<ElementId>) {
        self.focus = Some(FocusRequest::Focus(id.into()));
    }

    /// Tira o foco de qualquer elemento.
    pub fn blur(&mut self) {
        self.focus = Some(FocusRequest::Blur);
    }

    /// Foco no próximo/anterior focável, como Tab/Shift+Tab.
    pub fn focus_next(&mut self) {
        self.focus = Some(FocusRequest::Next);
    }

    pub fn focus_prev(&mut self) {
        self.focus = Some(FocusRequest::Prev);
    }

    /// Reconstrói de novo no próximo frame (~16 ms), mesmo sem evento.
    pub fn request_animation_frame(&mut self) {
        self.animation_frame = true;
    }

    /// Roda `f` uma vez depois de `delay` (ex.: fechar toast, debounce).
    /// O laço só acorda para isso; ocioso continua sem gastar CPU.
    pub fn spawn_timer<V: 'static>(&mut self, delay: Duration, f: impl FnOnce(&mut V, &mut EventCx) + 'static) -> TimerId {
        let id = TimerId::next();
        let erased: TimerFn = Box::new(move |v: &mut dyn Any, cx: &mut EventCx| {
            if let Some(v) = v.downcast_mut::<V>() {
                f(v, cx);
            }
        });
        self.timers.push((delay, id, erased));
        id
    }

    /// Cancela um timer ainda pendente (sem efeito se já rodou).
    pub fn cancel_timer(&mut self, id: TimerId) {
        self.cancelled.push(id);
    }

    /// Roda `work` numa thread do pool e depois `done` na thread da UI com a view.
    /// Para código async, `work` pode fazer `runtime.block_on(fut)`; a lib não tem runtime.
    /// Se a janela fechar antes, o resultado é descartado.
    pub fn spawn<V: 'static, T: Send + 'static>(
        &mut self,
        work: impl FnOnce() -> T + Send + 'static,
        done: impl FnOnce(&mut V, T, &mut EventCx) + 'static,
    ) -> TaskId {
        let req = super::task::request(work, done);
        let id = req.id;
        self.tasks.push(req);
        id
    }

    /// Descarta o resultado de uma tarefa (o `work` em andamento termina, mas `done` não roda).
    pub fn cancel_task(&mut self, id: TaskId) {
        self.cancelled_tasks.push(id);
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use crate::gfx::WindowCommand;

    #[test]
    fn window_command_queued() {
        let mut cx = EventCx::default();
        cx.window_command(WindowCommand::Minimize);
        cx.window_command(WindowCommand::Close);
        assert_eq!(cx.window_commands, vec![WindowCommand::Minimize, WindowCommand::Close]);
    }
}
