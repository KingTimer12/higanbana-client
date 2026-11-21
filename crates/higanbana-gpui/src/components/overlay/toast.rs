//! Toast e a fila `Toasts` guardada na view. Cada toast vence `duration` depois
//! de entrar; o `Toaster` esconde os vencidos e agenda o próximo redesenho no
//! prazo (sem timers nem polling). Vencidos saem da memória no próximo `push`.
use crate::components::Variant;
use std::borrow::Cow;
use std::time::{Duration, Instant};

/// Identificador de um toast na fila (para `dismiss`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(pub(crate) u64);

/// Notificação curta: título, descrição opcional, variante e duração.
#[derive(Debug, Clone)]
pub struct Toast {
    pub title: Cow<'static, str>,
    pub description: Option<Cow<'static, str>>,
    /// `Default` ou `Destructive` (as demais são tratadas como `Default`).
    pub variant: Variant,
    pub duration: Duration,
    /// Tom da notificação do kit (ícone colorido); só o `Notifier` usa.
    pub tone: Option<ToastTone>,
}

/// Tom das notificações do kit: define ícone e cor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastTone {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastTone {
    /// Ícone e cor (kit: info, circle-check, triangle-alert, circle-x).
    pub fn look(self, t: &crate::theme::Theme) -> (&'static str, crate::color::Color) {
        match self {
            ToastTone::Info => ("info", t.info),
            ToastTone::Success => ("circle-check", t.success),
            ToastTone::Warning => ("triangle-alert", t.warning),
            ToastTone::Error => ("circle-x", t.destructive),
        }
    }
}

/// Duração das notificações do kit.
pub const NOTIFICATION_DURATION: Duration = Duration::from_secs(5);

impl Toast {
    pub fn new(title: impl Into<Cow<'static, str>>) -> Self {
        Self { title: title.into(), description: None, variant: Variant::Default, duration: Duration::from_secs(4), tone: None }
    }

    pub fn tone(mut self, t: ToastTone) -> Self {
        self.tone = Some(t);
        self
    }

    pub fn description(mut self, d: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(d.into());
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = d;
        self
    }
}

pub(super) struct Entry {
    pub id: ToastId,
    pub toast: Toast,
    pub expires: Instant,
}

/// Fila de toasts (mais antigo primeiro). Mostra no máximo `max_visible`.
pub struct Toasts {
    pub(super) items: Vec<Entry>,
    next: u64,
    pub(super) max_visible: usize,
}

impl Default for Toasts {
    fn default() -> Self {
        Self { items: Vec::new(), next: 1, max_visible: 3 }
    }
}

impl Toasts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Quantos aparecem ao mesmo tempo (os mais recentes).
    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n.max(1);
        self
    }

    /// Enfileira um toast agora.
    pub fn push(&mut self, toast: Toast) -> ToastId {
        self.push_at(toast, Instant::now())
    }

    /// Como `push`, com o instante explícito (testes, relógio próprio).
    pub fn push_at(&mut self, toast: Toast, now: Instant) -> ToastId {
        self.prune(now);
        let id = ToastId(self.next);
        self.next += 1;
        let expires = now + toast.duration;
        self.items.push(Entry { id, toast, expires });
        id
    }

    /// Notificação de sucesso no visual do kit (5 s).
    pub fn push_success(&mut self, message: impl Into<Cow<'static, str>>) -> ToastId {
        self.push(Toast::new(message).tone(ToastTone::Success).duration(NOTIFICATION_DURATION))
    }

    /// Notificação de erro no visual do kit (5 s).
    pub fn push_error(&mut self, message: impl Into<Cow<'static, str>>) -> ToastId {
        self.push(Toast::new(message).tone(ToastTone::Error).duration(NOTIFICATION_DURATION))
    }

    /// Fecha um toast (botão "×").
    pub fn dismiss(&mut self, id: ToastId) {
        self.items.retain(|e| e.id != id);
    }

    /// Esquece os vencidos.
    pub fn prune(&mut self, now: Instant) {
        self.items.retain(|e| e.expires > now);
    }

    /// Vivos em `now`, mais recente primeiro, limitados a `max_visible`.
    pub(super) fn visible(&self, now: Instant) -> impl Iterator<Item = &Entry> {
        self.items.iter().rev().filter(move |e| e.expires > now).take(self.max_visible)
    }

    /// Ids visíveis em `now` (mais recente primeiro).
    pub fn visible_ids(&self, now: Instant) -> Vec<ToastId> {
        self.visible(now).map(|e| e.id).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
