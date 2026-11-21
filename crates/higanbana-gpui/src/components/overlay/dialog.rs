//! Dialog (shadcn): modal centralizado com fundo escurecido, título, descrição,
//! conteúdo, rodapé e botão "×". Escape, clique fora e "×" pedem fechamento.
use crate::components::basic::Component;
use super::common::{close_button, nothing, panel, presence, sub, OnChange};
use crate::color::rgba;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, overlay, text, Anchor, Element, ElementId};
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

/// Diálogo modal. `open` vem da view; mudanças chegam por `on_open_change`.
pub struct Dialog<V> {
    id: ElementId,
    open: bool,
    on_open_change: Option<OnChange<V, bool>>,
    title: Option<Cow<'static, str>>,
    description: Option<Cow<'static, str>>,
    body: Vec<Element<V>>,
    footer: Vec<Element<V>>,
    width: f32,
}

impl<V: 'static> Dialog<V> {
    pub fn new(id: impl Into<ElementId>, open: bool) -> Self {
        let id = id.into();
        Self { id, open, on_open_change: None, title: None, description: None, body: Vec::new(), footer: Vec::new(), width: 512.0 }
    }

    /// Id do botão "×" (recebe o foco ao abrir via [`Dialog::open`]).
    pub fn close_id(id: impl Into<ElementId>) -> ElementId {
        sub(id.into(), "fechar")
    }

    /// Ajuda para o handler que abre o diálogo: move o foco para dentro dele.
    pub fn open(cx: &mut EventCx, id: impl Into<ElementId>) {
        cx.focus(Self::close_id(id));
        cx.notify();
    }

    pub fn title(mut self, t: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Texto secundário abaixo do título (`muted_foreground`).
    pub fn description(mut self, d: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Conteúdo principal (empilhado em coluna).
    pub fn child(mut self, e: impl Into<Element<V>>) -> Self {
        self.body.push(e.into());
        self
    }

    /// Ações do rodapé (alinhadas à direita).
    pub fn footer(mut self, e: impl Into<Element<V>>) -> Self {
        self.footer.push(e.into());
        self
    }

    /// Largura máxima do painel (padrão 512, `max-w-lg`).
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Chamado com `false` no Escape, clique fora ou "×".
    pub fn on_open_change(mut self, f: impl Fn(&mut V, bool, &mut EventCx) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(f));
        self
    }

}

impl<V: 'static> Component<V> for Dialog<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let t = presence(cx, self.id, self.open);
        if t <= 0.001 {
            return nothing();
        }
        let theme = cx.theme();
        let ring = cx.focus().visible;
        let change = self.on_open_change;
        let close = move |v: &mut V, cx: &mut EventCx| {
            if let Some(f) = &change {
                f(v, false, cx);
            }
            cx.notify();
        };
        let close_btn = {
            let close = close.clone();
            close_button(Self::close_id(self.id), theme, ring).on_click(move |v, _, cx| close(v, cx))
        };
        let mut header = div().flex_col().gap(6.0).pr(24.0);
        if let Some(title) = self.title {
            header = header.child(text(title).text_size(theme.typography.lg).font_weight(600).text_color(theme.foreground));
        }
        if let Some(d) = self.description {
            header = header.child(text(d).text_color(theme.muted_foreground));
        }
        let mut p = panel(theme)
            .id(self.id)
            .relative()
            .w(self.width)
            .p(24.0)
            .gap(16.0)
            .bg(theme.background)
            .mt(8.0 * (1.0 - t))
            .opacity(t)
            .child(header)
            .children(self.body);
        if !self.footer.is_empty() {
            p = p.child(div().justify_end().gap(8.0).children(self.footer));
        }
        p = p.child(div().absolute().top(16.0).right(16.0).child(close_btn));
        let o = overlay(Anchor::center(), p).backdrop(rgba(0x0000_00cc).fade(t));
        // Fechando: sem modal nem dismiss, só a animação de saída.
        if self.open { o.modal().on_dismiss(close).into() } else { o.into() }
    }
}
