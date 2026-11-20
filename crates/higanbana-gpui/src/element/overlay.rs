//! Overlays (portais): conteúdo pintado numa camada acima da árvore inteira —
//! base para Dialog, Select, Tooltip, Toast e menus.
use super::handlers::DismissHandler;
use super::{Element, ElementId, Kind};
use crate::color::Color;
use crate::cx::EventCx;
use crate::geometry::Point;

/// Lado do elemento âncora onde o overlay aparece (vira para o oposto se não couber).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// Alinhamento ao longo do lado da âncora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnchorAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Onde posicionar o overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Anchor {
    /// Frações 0..1 da janela; o mesmo ponto relativo do overlay coincide
    /// (0.5, 0.5 = centro; 1, 1 = canto inferior direito).
    Window { x: f32, y: f32 },
    /// Junto da caixa do elemento `id` (precisa estar na árvore).
    Element { id: ElementId, side: Side, align: AnchorAlign },
    /// Canto superior esquerdo num ponto da janela (menu de contexto).
    Point(Point),
}

impl Anchor {
    pub fn center() -> Self {
        Anchor::Window { x: 0.5, y: 0.5 }
    }

    pub fn window(x: f32, y: f32) -> Self {
        Anchor::Window { x, y }
    }

    pub fn below(id: impl Into<ElementId>) -> Self {
        Anchor::Element { id: id.into(), side: Side::Bottom, align: AnchorAlign::Start }
    }

    pub fn above(id: impl Into<ElementId>) -> Self {
        Anchor::Element { id: id.into(), side: Side::Top, align: AnchorAlign::Start }
    }

    pub fn left_of(id: impl Into<ElementId>) -> Self {
        Anchor::Element { id: id.into(), side: Side::Left, align: AnchorAlign::Start }
    }

    pub fn right_of(id: impl Into<ElementId>) -> Self {
        Anchor::Element { id: id.into(), side: Side::Right, align: AnchorAlign::Start }
    }

    pub fn at(p: Point) -> Self {
        Anchor::Point(p)
    }

    /// Troca o alinhamento (só para `Anchor::Element`).
    pub fn align(self, align: AnchorAlign) -> Self {
        match self {
            Anchor::Element { id, side, .. } => Anchor::Element { id, side, align },
            other => other,
        }
    }
}

/// Configuração do overlay, guardada no nó raiz dele.
pub(crate) struct OverlaySpec<V> {
    pub anchor: Anchor,
    /// Distância da âncora (ou margem da janela).
    pub gap: f32,
    pub modal: bool,
    pub backdrop: Option<Color>,
    pub match_width: bool,
    pub on_dismiss: Option<DismissHandler<V>>,
}

/// Construtor de overlay; vira `Element` ao ser passado para `.child(...)`.
pub struct Overlay<V> {
    spec: OverlaySpec<V>,
    content: Element<V>,
}

/// Overlay com `content` posicionado por `anchor`. Pode ser filho de qualquer
/// elemento: sai do fluxo e é pintado acima de tudo (overlays posteriores por cima).
pub fn overlay<V>(anchor: Anchor, content: impl Into<Element<V>>) -> Overlay<V> {
    let spec = OverlaySpec { anchor, gap: 4.0, modal: false, backdrop: None, match_width: false, on_dismiss: None };
    Overlay { spec, content: content.into() }
}

impl<V> Overlay<V> {
    /// Distância da âncora em pixels lógicos (padrão 4).
    pub fn gap(mut self, gap: f32) -> Self {
        self.spec.gap = gap;
        self
    }

    /// Bloqueia entrada no conteúdo abaixo e prende o Tab dentro do overlay.
    pub fn modal(mut self) -> Self {
        self.spec.modal = true;
        self
    }

    /// Cor pintada sobre a janela inteira, abaixo do overlay (escurecer o fundo).
    pub fn backdrop(mut self, color: Color) -> Self {
        self.spec.backdrop = Some(color);
        self
    }

    /// Largura mínima = largura da âncora (dropdown de Select).
    pub fn match_width(mut self) -> Self {
        self.spec.match_width = true;
        self
    }

    /// Clique fora (exceto na âncora) ou Escape não tratado: o app deve fechar o overlay.
    pub fn on_dismiss(mut self, f: impl Fn(&mut V, &mut EventCx) + 'static) -> Self {
        self.spec.on_dismiss = Some(Box::new(f));
        self
    }
}

impl<V> From<Overlay<V>> for Element<V> {
    fn from(o: Overlay<V>) -> Self {
        let mut e = super::element(Kind::Overlay(Box::new(o.spec)), Default::default());
        e.children.push(o.content);
        e
    }
}
