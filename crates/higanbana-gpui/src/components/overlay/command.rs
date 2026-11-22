//! Command (kit): campo de busca + resultados em grupos, navegação por setas.
//! Não filtra sozinho: o app recebe a consulta (`on_query`) e manda os grupos.
use super::command_state::{clamp, step, CommandState};
use super::common::sub;
use crate::components::basic::{Component, Spinner};
use crate::components::form::input_keys::{attach, Callbacks};
use crate::components::form::password::patch_field;
use crate::cx::{EventCx, RenderCx};
use crate::element::{div, scroll, text, text_field, Element, ElementId};
use crate::icon::icon;
use crate::input::Key;
use crate::style::Styled;
use std::borrow::Cow;
use std::rc::Rc;

type Lens<V> = Rc<dyn Fn(&mut V) -> &mut CommandState>;
type QueryFn<V> = Rc<dyn Fn(&mut V, &str, &mut EventCx)>;
type ConfirmFn<V> = Rc<dyn Fn(&mut V, (usize, usize), &mut EventCx)>;

/// Lente do campo de texto dentro do estado.
fn input_lens<V>(l: Lens<V>) -> impl Fn(&mut V) -> &mut crate::text_input::TextInputState + Clone {
    move |v| &mut l(v).input
}

/// Linha de resultado com conteúdo livre.
pub struct CommandItem<V> {
    id: ElementId,
    children: Vec<Element<V>>,
}

impl<V: 'static> CommandItem<V> {
    pub fn new(id: impl Into<ElementId>) -> Self {
        CommandItem { id: id.into(), children: Vec::new() }
    }

    pub fn child(mut self, e: impl Into<Element<V>>) -> Self {
        self.children.push(e.into());
        self
    }
}

/// Grupo com título (ex.: "Animes").
pub struct CommandGroup<V> {
    label: Cow<'static, str>,
    items: Vec<CommandItem<V>>,
}

impl<V: 'static> CommandGroup<V> {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        CommandGroup { label: label.into(), items: Vec::new() }
    }

    pub fn item(mut self, it: CommandItem<V>) -> Self {
        self.items.push(it);
        self
    }
}

/// `Command::new("busca", &v.cmd, |v| &mut v.cmd).group(..).on_confirm(..)`.
pub struct Command<'a, V> {
    id: ElementId,
    state: &'a CommandState,
    lens: Lens<V>,
    placeholder: Cow<'static, str>,
    max_h: f32,
    groups: Vec<CommandGroup<V>>,
    empty: Option<Element<V>>,
    on_query: Option<QueryFn<V>>,
    on_confirm: Option<ConfirmFn<V>>,
}

impl<'a, V: 'static> Command<'a, V> {
    pub fn new(id: impl Into<ElementId>, state: &'a CommandState, lens: impl Fn(&mut V) -> &mut CommandState + 'static) -> Self {
        Command {
            id: id.into(),
            state,
            lens: Rc::new(lens),
            placeholder: Cow::Borrowed(""),
            // Kit: 18.75 rem.
            max_h: 300.0,
            groups: Vec::new(),
            empty: None,
            on_query: None,
            on_confirm: None,
        }
    }

    pub fn placeholder(mut self, p: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn max_h(mut self, h: f32) -> Self {
        self.max_h = h;
        self
    }

    pub fn group(mut self, g: CommandGroup<V>) -> Self {
        self.groups.push(g);
        self
    }

    /// Mostrado quando não há itens e não está buscando.
    pub fn empty(mut self, e: impl Into<Element<V>>) -> Self {
        self.empty = Some(e.into());
        self
    }

    pub fn on_query(mut self, f: impl Fn(&mut V, &str, &mut EventCx) + 'static) -> Self {
        self.on_query = Some(Rc::new(f));
        self
    }

    pub fn on_confirm(mut self, f: impl Fn(&mut V, (usize, usize), &mut EventCx) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(f));
        self
    }

    /// Id do campo (para focar ao abrir).
    pub fn input_id(id: impl Into<ElementId>) -> ElementId {
        sub(id.into(), "campo")
    }

    /// Id do indicador de busca em andamento.
    pub fn spinner_id(id: impl Into<ElementId>) -> ElementId {
        sub(id.into(), "buscando")
    }

    fn list_id(id: ElementId) -> ElementId {
        sub(id, "lista")
    }
}

impl<V: 'static> Component<V> for Command<'_, V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let id = self.id;
        let sizes: Rc<[usize]> = self.groups.iter().map(|g| g.items.len()).collect();
        let current = clamp(self.state.selected, &sizes);
        let item_ids: Rc<Vec<Vec<ElementId>>> = Rc::new(self.groups.iter().map(|g| g.items.iter().map(|i| i.id).collect()).collect());
        let t = cx.theme();
        let (muted, accent, accent_fg) = (t.muted_foreground, t.accent, t.accent_foreground);
        let (sm, xs, radius, border) = (t.typography.sm, t.typography.xs, t.radius, t.border);

        // Campo: lupa + texto + indicador, linha embaixo.
        let field_lens = input_lens(self.lens.clone());
        let mut field = text_field(Self::input_id(id), &self.state.input, field_lens.clone())
            .flex_1()
            .h(36.0)
            .text_size(sm)
            .text_color(t.popover_foreground)
            .placeholder_color(muted)
            .selection_color(t.primary.alpha(0.35))
            .caret_color(t.foreground);
        patch_field(&mut field, &self.state.input, false, Some(self.placeholder.clone()));
        let change = self.on_query.clone().map(|f| -> Rc<dyn Fn(&mut V, &str, &mut EventCx)> {
            Rc::new(move |v: &mut V, q: &str, cx: &mut EventCx| f(v, q, cx))
        });
        let mut field = attach(field, field_lens, false, Callbacks { change, submit: None });
        // Setas e Enter vão para a lista antes do campo (que usaria Up/Down como Home/End).
        let (lens, confirm) = (self.lens.clone(), self.on_confirm.clone());
        let list_id = Self::list_id(id);
        let (sizes_k, ids_k) = (sizes.clone(), item_ids.clone());
        let keys = move |v: &mut V, ev: &crate::input::KeyEvent, cx: &mut EventCx| {
            let st = lens(v);
            let (sizes, item_ids) = (&sizes_k, &ids_k);
            let Some(cur) = clamp(st.selected, sizes) else { return };
            match ev.key {
                Key::Down | Key::Up => {
                    let next = step(cur, sizes, ev.key == Key::Down);
                    st.selected = Some(next);
                    if let Some(child) = item_ids.get(next.0).and_then(|g| g.get(next.1)) {
                        cx.scroll_into_view(list_id, *child);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
                Key::Enter => {
                    st.selected = Some(cur);
                    cx.stop_propagation();
                    if let Some(f) = &confirm {
                        f(v, cur, cx);
                    }
                }
                _ => {}
            }
        };
        let edit = field.interact().on_key_down.take();
        field = field.on_key_down(move |v, ev, cx| {
            keys(v, ev, cx);
            if !cx.propagation_stopped()
                && let Some(h) = &edit {
                    h(v, ev, cx);
                }
        });
        let mut bar = div().flex_shrink_0().items_center().gap(8.0).px(12.0).child(icon("search").size(16.0).text_color(muted)).child(field);
        if self.state.loading {
            bar = bar.child(div().id(Self::spinner_id(id)).child(Spinner::new(sub(id, "giro")).size(16.0).color(muted).render(cx)));
        }

        // Lista.
        let mut list = scroll(Self::list_id(id)).flex_col().max_h(self.max_h).p(4.0);
        let has_rows = sizes.iter().any(|&n| n > 0);
        for (g, group) in self.groups.into_iter().enumerate() {
            if group.items.is_empty() {
                continue;
            }
            list = list.child(div().w_full().px(8.0).py(6.0).text_size(xs).font_weight(500).text_color(muted).child(text(group.label)));
            for (i, item) in group.items.into_iter().enumerate() {
                let selected = current == Some((g, i));
                let (l1, l2, confirm) = (self.lens.clone(), self.lens.clone(), self.on_confirm.clone());
                let mut row = div()
                    .id(item.id)
                    .w_full()
                    .items_center()
                    .gap(8.0)
                    .px(8.0)
                    .py(6.0)
                    .text_size(sm)
                    .rounded(radius)
                    .cursor_pointer()
                    .on_hover(move |v: &mut V, on, cx| {
                        let st = l1(v);
                        if on && st.selected != Some((g, i)) {
                            st.selected = Some((g, i));
                            cx.notify();
                        }
                    })
                    .on_click(move |v: &mut V, _, cx| {
                        l2(v).selected = Some((g, i));
                        if let Some(f) = &confirm {
                            f(v, (g, i), cx);
                        }
                    });
                if selected {
                    row = row.bg(accent).text_color(accent_fg);
                }
                list = list.child(row.children(item.children));
            }
        }
        let body = if has_rows {
            list
        } else if self.state.loading {
            div()
        } else {
            let empty = self.empty.unwrap_or_else(|| text("Nenhum resultado"));
            div().w_full().p(4.0).child(div().w_full().justify_center().py(24.0).text_size(sm).text_color(muted).child(empty))
        };

        div()
            .id(id)
            .flex_col()
            .w_full()
            .overflow_hidden()
            .bg(t.popover)
            .text_color(t.popover_foreground)
            .child(bar)
            .child(div().h(1.0).w_full().flex_shrink_0().bg(border))
            .child(body)
    }
}
