//! Popover: painel flutuante preso a um gatilho. Abre/fecha no clique do gatilho,
//! fecha com Esc ou clique fora, entra/sai com fade + leve deslize (como o kit).
use super::common::{sub, Lens, PRESENCE};
use crate::components::basic::Component;
use crate::cx::RenderCx;
use crate::element::{div, overlay, Anchor, AnchorAlign, Element, ElementId, Side};
use crate::style::Styled;
use std::rc::Rc;

/// Estado na view: `open` pode ser mudado direto (`state.open = false`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PopoverState {
    pub open: bool,
}

/// Canto do gatilho onde o painel se prende. `Bottom*` = painel acima do gatilho
/// (barra de controles no rodapé); `Top*` = abaixo. Vira se não couber.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PopoverAnchor {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

type ContentFn<V> = Box<dyn FnOnce(&mut RenderCx<'_, V>) -> Element<V>>;

/// `Popover::new("faixas", &v.pop, |v| &mut v.pop, botao).anchor(BottomRight).w(288.).content(|cx| ...)`.
pub struct Popover<V> {
    id: ElementId,
    state: PopoverState,
    lens: Lens<V, PopoverState>,
    trigger: Element<V>,
    anchor: PopoverAnchor,
    width: Option<f32>,
    padding: f32,
    content: Option<ContentFn<V>>,
}

/// Deslize da entrada (px) e distância do gatilho.
const SLIDE: f32 = 4.0;
const GAP: f32 = 4.0;

impl<V: 'static> Popover<V> {
    pub fn new(
        id: impl Into<ElementId>,
        state: &PopoverState,
        lens: impl Fn(&mut V) -> &mut PopoverState + 'static,
        trigger: impl Into<Element<V>>,
    ) -> Self {
        Popover { id: id.into(), state: *state, lens: Rc::new(lens), trigger: trigger.into(), anchor: PopoverAnchor::default(), width: None, padding: 12.0, content: None }
    }

    pub fn anchor(mut self, a: PopoverAnchor) -> Self {
        self.anchor = a;
        self
    }

    /// Largura fixa do painel (sem ela, a do conteúdo).
    pub fn w(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Espaço interno do painel (padrão 12; menus usam 4).
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = p;
        self
    }

    /// Conteúdo do painel; só é construído enquanto visível.
    pub fn content(mut self, f: impl FnOnce(&mut RenderCx<'_, V>) -> Element<V> + 'static) -> Self {
        self.content = Some(Box::new(f));
        self
    }

    /// Id do painel (testes, `scroll_into_view`).
    pub fn panel_id(id: impl Into<ElementId>) -> ElementId {
        sub(id.into(), "painel")
    }
}

impl<V: 'static> Component<V> for Popover<V> {
    fn render(self, cx: &mut RenderCx<'_, V>) -> Element<V> {
        let id = self.id;
        let toggle = self.lens.clone();
        let trigger = div().track_focus(id).cursor_pointer().child(self.trigger).on_click(move |v: &mut V, _, cx| {
            let s = toggle(v);
            s.open = !s.open;
            cx.notify();
        });
        // `None` só depois que a saída termina: até lá o painel continua na árvore.
        let Some(p) = cx.presence(sub(id, "presenca"), self.state.open, PRESENCE) else { return trigger };
        let Some(content) = self.content else { return trigger };
        let theme = cx.theme();
        let (side, align, dy) = match self.anchor {
            PopoverAnchor::TopLeft => (Side::Bottom, AnchorAlign::Start, -SLIDE),
            PopoverAnchor::TopRight => (Side::Bottom, AnchorAlign::End, -SLIDE),
            PopoverAnchor::BottomLeft => (Side::Top, AnchorAlign::Start, SLIDE),
            PopoverAnchor::BottomRight => (Side::Top, AnchorAlign::End, SLIDE),
        };
        let panel = div()
            .id(Self::panel_id(id))
            .flex_col()
            .p(self.padding)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .text_size(theme.typography.sm)
            .border(1.0, theme.border)
            .rounded(theme.radius)
            .shadow(crate::color::rgba(0x0000_0059), 8.0)
            .opacity(p)
            .translate_y(dy * (1.0 - p))
            .when_some(self.width, |e, w| e.w(w))
            .child(content(cx));
        let lens = self.lens;
        let anchor = Anchor::Element { id, side, align };
        let menu = overlay(anchor, panel).gap(GAP).on_dismiss(move |v: &mut V, cx| {
            lens(v).open = false;
            cx.notify();
        });
        trigger.child(menu)
    }
}
