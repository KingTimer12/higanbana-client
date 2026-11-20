//! Select (shadcn): gatilho como input com chevron; lista flutuante abaixo
//! (mesma largura, vira para cima se não couber), check no escolhido,
//! destaque por mouse/teclado (Cima/Baixo/Enter/Escape e type-ahead) e rolagem.
use crate::components::basic::Component;
use super::common::{panel, ring, sub, sub_i, Lens, OnChange};
use super::select_state::{KeyOutcome, SelectState};
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, overlay, scroll, text, Anchor, Element, ElementId};
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

/// Seleção de um item de uma lista de rótulos.
pub struct Select<V> {
    id: ElementId,
    state: SelectState,
    lens: Lens<V, SelectState>,
    items: Vec<Cow<'static, str>>,
    placeholder: Cow<'static, str>,
    width: f32,
    max_height: f32,
    on_change: Option<OnChange<V, usize>>,
}

impl<V: 'static> Select<V> {
    /// `state` é uma cópia do estado atual; `lens` aponta para ele na view.
    pub fn new(id: impl Into<ElementId>, state: &SelectState, lens: impl Fn(&mut V) -> &mut SelectState + 'static) -> Self {
        Self {
            id: id.into(),
            state: *state,
            lens: Rc::new(lens),
            items: Vec::new(),
            placeholder: Cow::Borrowed("Selecione..."),
            width: 180.0,
            max_height: 240.0,
            on_change: None,
        }
    }

    pub fn item(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.items.push(label.into());
        self
    }

    pub fn items<S: Into<Cow<'static, str>>>(mut self, labels: impl IntoIterator<Item = S>) -> Self {
        self.items.extend(labels.into_iter().map(Into::into));
        self
    }

    pub fn placeholder(mut self, p: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Altura máxima da lista antes de rolar (padrão 240).
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = h;
        self
    }

    /// Chamado quando um item é escolhido (clique ou Enter).
    pub fn on_change(mut self, f: impl Fn(&mut V, usize, &mut EventCx) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Id do item `i` da lista.
    pub fn item_id(id: impl Into<ElementId>, i: usize) -> ElementId {
        sub_i(id.into(), "item", i)
    }

}

impl<V: 'static> Component<V> for Select<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let theme = cx.theme();
        let (id, st, n) = (self.id, self.state, self.items.len());
        let label = st.selected.and_then(|i| self.items.get(i)).cloned();
        let has_value = label.is_some();
        let labels: Rc<[Cow<'static, str>]> = self.items.into();
        let (lens, change) = (self.lens.clone(), self.on_change.clone());
        let (lens_c, lens_t) = (self.lens.clone(), self.lens.clone());
        let typed = labels.clone();
        let trigger = div()
            .track_focus(id)
            .w(self.width)
            .h(36.0)
            .px(12.0)
            .items_center()
            .justify_between()
            .gap(8.0)
            .rounded(theme.radius - 2.0)
            .border(1.0, theme.input)
            .text_size(theme.typography.sm)
            .cursor_pointer()
            .on_click(move |v: &mut V, _, cx| {
                lens_c(v).toggle();
                cx.notify();
            })
            .on_key_down(move |v: &mut V, ev, cx| match lens(v).key(ev.key, n) {
                KeyOutcome::Ignored => {}
                out => {
                    if let (KeyOutcome::Chose(i), Some(f)) = (out, &change) {
                        f(v, i, cx);
                    }
                    cx.notify();
                    cx.stop_propagation();
                }
            })
            .on_char(move |v: &mut V, c, cx| {
                if lens_t(v).type_ahead(c, typed.iter().map(|s| s.as_ref())) {
                    cx.notify();
                    cx.stop_propagation();
                }
            })
            .child(match label {
                Some(l) => text(l),
                None => text(self.placeholder).text_color(theme.muted_foreground),
            })
            .child(text("⌄").text_color(theme.muted_foreground));
        let trigger = ring(trigger, theme, cx.focus().visible).when(has_value, |e| e.text_color(theme.foreground));
        if !st.open || n == 0 {
            return trigger;
        }
        let mut list = scroll(sub(id, "lista")).max_h(self.max_height).p(4.0);
        for (i, l) in labels.iter().enumerate() {
            list = list.child(item(id, i, l.clone(), st, theme, &self.lens, &self.on_change));
        }
        let lens = self.lens;
        let menu = overlay(Anchor::below(id), panel(theme).child(list)).match_width().on_dismiss(move |v: &mut V, cx| {
            lens(v).close();
            cx.notify();
        });
        trigger.child(menu)
    }
}

/// Linha da lista: destaque `accent`, check no escolhido.
fn item<V: 'static>(
    id: ElementId,
    i: usize,
    label: Cow<'static, str>,
    st: SelectState,
    theme: &crate::theme::Theme,
    lens: &Lens<V, SelectState>,
    change: &Option<OnChange<V, usize>>,
) -> Element<V> {
    let (lc, lh, change) = (lens.clone(), lens.clone(), change.clone());
    let row = div()
        .id(Select::<V>::item_id(id, i))
        .h(32.0)
        .px(8.0)
        .items_center()
        .justify_between()
        .rounded(theme.radius - 4.0)
        .cursor_pointer()
        .on_click(move |v: &mut V, _, cx| {
            lc(v).choose(i);
            if let Some(f) = &change {
                f(v, i, cx);
            }
            cx.notify();
        })
        .on_hover(move |v: &mut V, on, cx| {
            if on {
                lh(v).highlighted = i;
                cx.notify();
            }
        })
        .child(text(label))
        .when(st.selected == Some(i), |e| e.child(text("✓")));
    row.when(st.highlighted == i, |e| e.bg(theme.accent).text_color(theme.accent_foreground))
}
