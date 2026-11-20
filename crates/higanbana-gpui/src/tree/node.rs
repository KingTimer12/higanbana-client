//! Nós achatados em pré-ordem: a subárvore de `i` ocupa `i..end`.
//! Overlays viram raízes extras depois da árvore principal (por cima dela).
use crate::element::{ClickHandler, Element, ElementId, FieldData, HoverStyle, ImageSource, Interactivity, Kind, OverlaySpec};
use crate::geometry::Rect;
use crate::style::{Style, TextStyle};
use std::borrow::Cow;

pub(crate) const NO_PARENT: u32 = u32::MAX;

/// Campo de texto já medido: x de cada fronteira de caractere e rolagem horizontal.
pub(crate) struct FieldNode {
    pub data: FieldData,
    pub carets: Vec<f32>,
    pub scroll: f32,
    /// Altura de uma linha (texto centralizado verticalmente no campo).
    pub line_h: f32,
}

pub(crate) enum NodeKind {
    Div,
    Text(Cow<'static, str>),
    Image(ImageSource),
    TextField(Box<FieldNode>),
}

pub(crate) struct Node<V> {
    pub kind: NodeKind,
    pub style: Style,
    /// Estilo de texto já herdado (usado por nós de texto).
    pub text: TextStyle,
    pub id: Option<ElementId>,
    pub hover: Option<HoverStyle>,
    pub on_click: Option<ClickHandler<V>>,
    pub interact: Option<Box<Interactivity<V>>>,
    /// Este nó ou um ancestral está desabilitado.
    pub disabled: bool,
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

/// Raiz de overlay: índice do nó e a configuração.
pub(crate) struct OverlayRoot<V> {
    pub root: u32,
    pub spec: Box<OverlaySpec<V>>,
}

type Deferred<V> = Vec<Option<(Box<OverlaySpec<V>>, Element<V>, TextStyle)>>;

/// Achata `root` em `out` e os overlays encontrados (em qualquer profundidade) em
/// raízes extras. Reaproveita a capacidade dos Vecs entre frames.
pub(crate) fn flatten<V>(root: Element<V>, base: &TextStyle, out: &mut Vec<Node<V>>, overlays: &mut Vec<OverlayRoot<V>>) {
    out.clear();
    overlays.clear();
    let mut deferred = Deferred::new();
    push(root, NO_PARENT, base, false, out, &mut deferred);
    let mut k = 0;
    // Overlays aninhados em overlays entram no fim da fila: ficam por cima.
    while k < deferred.len() {
        let (spec, content, text) = deferred[k].take().expect("cada overlay é visitado uma vez");
        overlays.push(OverlayRoot { root: out.len() as u32, spec });
        push(content, NO_PARENT, &text, false, out, &mut deferred);
        k += 1;
    }
}

fn push<V>(e: Element<V>, parent: u32, inherited: &TextStyle, disabled: bool, out: &mut Vec<Node<V>>, deferred: &mut Deferred<V>) {
    let kind = match e.kind {
        Kind::Overlay(spec) => {
            if let Some(content) = e.children.into_iter().next() {
                deferred.push(Some((spec, content, inherited.clone())));
            }
            return;
        }
        Kind::Div => NodeKind::Div,
        Kind::Text(s) => NodeKind::Text(s),
        Kind::Image(src) => NodeKind::Image(src),
        Kind::TextField(data) => NodeKind::TextField(Box::new(FieldNode { data: *data, carets: Vec::new(), scroll: 0.0, line_h: 0.0 })),
    };
    let index = out.len() as u32;
    let text = inherited.refined(&e.style.text);
    let disabled = disabled || e.interact.as_ref().is_some_and(|i| i.disabled);
    out.push(Node {
        kind,
        style: e.style,
        text,
        id: e.id,
        hover: e.hover,
        on_click: e.on_click,
        interact: e.interact,
        disabled,
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
            push(child, index, &inherited, disabled, out, deferred);
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
