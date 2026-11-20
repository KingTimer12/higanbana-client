//! Nós achatados em pré-ordem: a subárvore de `i` ocupa `i..end`.
use crate::element::{ClickHandler, Element, ElementId, HoverStyle, ImageSource, Kind};
use crate::geometry::Rect;
use crate::style::{Style, TextStyle};
use std::borrow::Cow;

pub(crate) const NO_PARENT: u32 = u32::MAX;

pub(crate) enum NodeKind {
    Div,
    Text(Cow<'static, str>),
    Image(ImageSource),
}

pub(crate) struct Node<V> {
    pub kind: NodeKind,
    pub style: Style,
    /// Estilo de texto já herdado (usado por nós de texto).
    pub text: TextStyle,
    pub id: Option<ElementId>,
    pub hover: Option<HoverStyle>,
    pub on_click: Option<ClickHandler<V>>,
    pub parent: u32,
    /// Fim (exclusivo) da subárvore.
    pub end: u32,
    /// Caixa de borda absoluta, já com a rolagem aplicada.
    pub rect: Rect,
    /// Caixa de conteúdo (sem borda/padding), absoluta.
    pub content: Rect,
    /// Região visível herdada dos ancestrais que recortam (para hit test).
    pub clip: Rect,
    /// Rolagem máxima (só contêineres de rolagem).
    pub scroll_max: f32,
}

/// Achata `root` em `out` (reaproveita a capacidade do Vec entre frames).
pub(crate) fn flatten<V>(root: Element<V>, base: &TextStyle, out: &mut Vec<Node<V>>) {
    out.clear();
    push(root, NO_PARENT, base, out);
}

fn push<V>(e: Element<V>, parent: u32, inherited: &TextStyle, out: &mut Vec<Node<V>>) {
    let index = out.len() as u32;
    let text = inherited.refined(&e.style.text);
    let kind = match e.kind {
        Kind::Div => NodeKind::Div,
        Kind::Text(s) => NodeKind::Text(s),
        Kind::Image(src) => NodeKind::Image(src),
    };
    out.push(Node {
        kind,
        style: e.style,
        text,
        id: e.id,
        hover: e.hover,
        on_click: e.on_click,
        parent,
        end: index + 1,
        rect: Rect::default(),
        content: Rect::default(),
        clip: Rect::default(),
        scroll_max: 0.0,
    });
    if !e.children.is_empty() {
        // Clona o estilo herdado só uma vez por pai com filhos.
        let inherited = out[index as usize].text.clone();
        for child in e.children {
            push(child, index, &inherited, out);
        }
    }
    out[index as usize].end = out.len() as u32;
}

/// Itera os filhos diretos de `i`.
pub(crate) fn children<V>(nodes: &[Node<V>], i: usize) -> impl Iterator<Item = usize> + '_ {
    let end = nodes[i].end as usize;
    let mut next = i + 1;
    std::iter::from_fn(move || {
        if next >= end {
            return None;
        }
        let c = next;
        next = nodes[c].end as usize;
        Some(c)
    })
}

/// `a` é `b` ou ancestral de `b`?
pub(crate) fn contains<V>(nodes: &[Node<V>], a: usize, b: usize) -> bool {
    a <= b && b < nodes[a].end as usize
}
