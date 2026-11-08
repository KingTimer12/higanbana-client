//! Interação com o ponteiro: cursor de mão sobre o que é clicável e um fade
//! curto no hover (borda ou fundo) em vez da troca seca do GPUI.
//!
//! - [`Button`]: o `Button` do kit com cursor de mão. O kit força a seta ao
//!   desenhar o botão; aqui pintamos o pedido de cursor depois dele (o último
//!   pedido sob o ponteiro vence), sem mexer no layout nem no clique.
//! - [`HoverFade`]: embrulha um elemento clicável e acende borda e/ou fundo
//!   com um fade de entrada e saída quando o ponteiro passa por cima.

use std::{panic::Location, time::{Duration, Instant}};

use gpui_kit::{
    AnyElement, App, Background, BorderStyle, Bounds, ClickEvent, Corners, CursorStyle, DispatchPhase, Element,
    ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, Pixels, SharedString, StyleRefinement, Styled, Window, px, quad, transparent_black,
};
use gpui_kit::component::{
    Disableable, Selectable, Sizable, Size,
    Icon,
    button::{Button as KitButton, ButtonRounded, ButtonVariant, ButtonVariants},
};

/// Duração do fade de hover: rápido o bastante para não atrasar o ponteiro.
const HOVER_FADE: Duration = Duration::from_millis(140);

// --- Cursor ---

/// Elemento transparente no layout: mede e pinta o filho como se fosse ele, e
/// depois pede o cursor de mão para a mesma área.
pub struct Pointer {
    child: AnyElement,
    on: bool,
}

impl Pointer {
    pub fn new(child: impl IntoElement, on: bool) -> Self {
        Self { child: child.into_any_element(), on }
    }
}

impl IntoElement for Pointer {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Pointer {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Option<Hitbox> {
        self.child.prepaint(window, cx);
        self.on.then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), hitbox: &mut Option<Hitbox>, window: &mut Window, cx: &mut App) {
        self.child.paint(window, cx);
        if let Some(h) = hitbox {
            window.set_cursor_style(CursorStyle::PointingHand, h);
        }
    }
}

/// O `Button` do kit, com cursor de mão quando dá para clicar.
///
/// Mesma API usada nas telas (variantes, tamanho, seleção, estilo); importe
/// este no lugar do `button::Button` do kit.
pub struct Button {
    inner: KitButton,
    disabled: bool,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { inner: KitButton::new(id), disabled: false }
    }

    fn map(mut self, f: impl FnOnce(KitButton) -> KitButton) -> Self {
        self.inner = f(self.inner);
        self
    }

    pub fn label(self, label: impl Into<SharedString>) -> Self {
        self.map(|b| b.label(label))
    }

    pub fn icon(self, icon: impl Into<Icon>) -> Self {
        let icon: Icon = icon.into();
        self.map(|b| b.icon(icon))
    }

    pub fn outline(self) -> Self {
        self.map(KitButton::outline)
    }

    pub fn tooltip(self, tooltip: impl Into<SharedString>) -> Self {
        self.map(|b| b.tooltip(tooltip))
    }

    pub fn rounded(self, rounded: impl Into<ButtonRounded>) -> Self {
        self.map(|b| b.rounded(rounded))
    }

    pub fn dropdown_caret(self, caret: bool) -> Self {
        self.map(|b| b.dropdown_caret(caret))
    }


    pub fn on_click(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.map(|b| b.on_click(handler))
    }
}

impl ButtonVariants for Button {
    fn with_variant(self, variant: ButtonVariant) -> Self {
        self.map(|b| b.with_variant(variant))
    }
}

impl Sizable for Button {
    fn with_size(self, size: impl Into<Size>) -> Self {
        let size = size.into();
        self.map(|b| b.with_size(size))
    }
}

impl Selectable for Button {
    fn selected(self, selected: bool) -> Self {
        self.map(|b| b.selected(selected))
    }

    fn is_selected(&self) -> bool {
        self.inner.is_selected()
    }

    fn open(self, open: bool) -> Self {
        self.map(|b| b.open(open))
    }

    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

impl Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.map(|b| b.disabled(disabled))
    }
}

impl gpui_kit::ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements);
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl IntoElement for Button {
    type Element = Pointer;

    fn into_element(self) -> Pointer {
        Pointer::new(self.inner, !self.disabled)
    }
}

/// Opções de um `RadioGroup` com cursor de mão em cada item (o grupo inteiro
/// não: o vão entre as opções não é clicável).
pub fn radios<S: Into<SharedString>>(labels: impl IntoIterator<Item = S>) -> impl Iterator<Item = gpui_kit::component::radio::Radio> {
    labels.into_iter().map(|l| {
        let l: SharedString = l.into();
        gpui_kit::component::radio::Radio::new(l.clone()).label(l).cursor_pointer()
    })
}

// --- Fade de hover ---

/// Avança o fade em direção a `target` (0 ou 1) proporcionalmente ao tempo:
/// interromper no meio volta do valor atual, sem pular.
fn fade_step(value: f32, target: f32, elapsed: Duration) -> f32 {
    let step = elapsed.as_secs_f32() / HOVER_FADE.as_secs_f32();
    if value < target { (value + step).min(target) } else { (value - step).max(target) }
}

#[derive(Clone, Copy)]
struct Fade {
    value: f32,
    at: Instant,
}

/// Embrulha um elemento e acende borda/fundo com fade quando o ponteiro entra,
/// apagando do mesmo jeito quando sai (a partir do valor atual, sem pulo).
pub struct HoverFade {
    id: ElementId,
    child: AnyElement,
    radius: Corners<Pixels>,
    border: Option<Hsla>,
    fill: Option<Hsla>,
}

/// `id` precisa ser estável (do domínio, não da posição na lista).
pub fn hover_fade(id: impl Into<ElementId>, child: impl IntoElement) -> HoverFade {
    HoverFade { id: id.into(), child: child.into_any_element(), radius: Corners::default(), border: None, fill: None }
}

/// Camada de hover como filho: cobre o pai inteiro (que precisa ser `relative`).
/// Como primeiro filho com `fill`, vira fundo; como último com `border`, contorno.
pub fn hover_layer(id: impl Into<ElementId>) -> HoverFade {
    hover_fade(id, gpui_kit::div().absolute().top_0().left_0().size_full())
}

impl HoverFade {
    pub fn radius(mut self, radius: impl Into<Corners<Pixels>>) -> Self {
        self.radius = radius.into();
        self
    }

    /// Cor da borda (1 px) no hover completo.
    pub fn border(mut self, color: Hsla) -> Self {
        self.border = Some(color);
        self
    }

    /// Cor do fundo no hover completo (pintado por baixo do conteúdo).
    pub fn fill(mut self, color: Hsla) -> Self {
        self.fill = Some(color);
        self
    }
}

impl IntoElement for HoverFade {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for HoverFade {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Hitbox {
        self.child.prepaint(window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(&mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), hitbox: &mut Hitbox, window: &mut Window, cx: &mut App) {
        let hovered = hitbox.is_hovered(window);
        let target = if hovered { 1. } else { 0. };
        let reduce = cx.reduce_motion();
        let value = window.with_element_state(id.expect("HoverFade tem id"), |state: Option<Fade>, _| {
            let now = Instant::now();
            let prev = state.unwrap_or(Fade { value: 0., at: now });
            let value = if reduce { target } else { fade_step(prev.value, target, now.duration_since(prev.at)) };
            (value, Fade { value, at: now })
        });
        if value != target {
            window.request_animation_frame();
        }
        // Curva de saída: acende rápido e assenta devagar.
        let eased = crate::motion::ease_out_cubic(value);

        if let Some(fill) = self.fill.filter(|_| eased > 0.) {
            window.paint_quad(quad(bounds, self.radius, Background::from(fill.opacity(eased)), px(0.), transparent_black(), BorderStyle::default()));
        }
        self.child.paint(window, cx);
        if let Some(border) = self.border.filter(|_| eased > 0.) {
            window.paint_quad(quad(bounds, self.radius, transparent_black(), px(1.), border.opacity(eased), BorderStyle::default()));
        }

        // Entrar ou sair redesenha (o GPUI só redesenha sozinho para `.hover()`).
        let hitbox = hitbox.clone();
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Capture && hitbox.is_hovered(window) != hovered {
                window.refresh();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_reaches_target_in_hover_duration() {
        assert_eq!(fade_step(0., 1., HOVER_FADE), 1.);
        assert_eq!(fade_step(1., 0., HOVER_FADE), 0.);
    }

    #[test]
    fn fade_is_gradual_and_reverses_from_current_value() {
        let half = fade_step(0., 1., HOVER_FADE / 2);
        assert!((half - 0.5).abs() < 0.01, "metade do tempo, metade do caminho: {half}");
        // Saiu no meio: começa a apagar de onde estava, não de 1.
        let back = fade_step(half, 0., HOVER_FADE / 4);
        assert!((back - 0.25).abs() < 0.01, "{back}");
    }
}
