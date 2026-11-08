//! Interação com o ponteiro: cursor de mão sobre o que é clicável e um fade
//! curto no hover (borda ou fundo) em vez da troca seca do GPUI.
//!
//! - [`Button`]: o `Button` do kit com cursor de mão. O kit força a seta ao
//!   desenhar o botão; aqui pintamos o pedido de cursor depois dele (o último
//!   pedido sob o ponteiro vence), sem mexer no layout nem no clique.
//! - [`HoverFade`]: embrulha um elemento clicável e acende borda e/ou fundo
//!   com um fade de entrada e saída quando o ponteiro passa por cima.

use std::{cell::Cell, panic::Location, rc::Rc, time::{Duration, Instant}};

use gpui_kit::{
    AnyElement, App, Background, BorderStyle, Bounds, ClickEvent, Corners, CursorStyle, DispatchPhase, Element, FocusHandle,
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

    /// Foco guardado por quem chama (para saber quando o botão está focado).
    pub fn track_focus(self, focus: &FocusHandle) -> Self {
        self.map(|b| b.track_focus(focus))
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

/// Avança `value` (0–1) em direção a `target` no ritmo em que o caminho
/// inteiro leva `duration`: interromper no meio volta do valor atual.
fn step_toward(value: f32, target: f32, elapsed: Duration, duration: Duration) -> f32 {
    let step = if duration.is_zero() { 1. } else { elapsed.as_secs_f32() / duration.as_secs_f32() };
    if value < target { (value + step).min(target) } else { (value - step).max(target) }
}

/// Progresso de um hover animado (0 = repouso, 1 = ponteiro em cima).
///
/// Fica num `Rc<Cell<_>>` no estado do elemento, dividido entre o desenho e o
/// listener do mouse: o listener roda fora do desenho, então ele troca o alvo
/// e pede o quadro, e esse quadro já anda para o alvo certo (um `refresh()`
/// durante o desenho não faz nada).
#[derive(Clone, Copy, Debug, PartialEq)]
struct HoverTrack {
    value: f32,
    /// Quando `value` foi calculado.
    at: Instant,
    hovered: bool,
}

impl HoverTrack {
    fn new(now: Instant) -> Self {
        Self { value: 0., at: now, hovered: false }
    }

    fn target(self) -> f32 {
        if self.hovered { 1. } else { 0. }
    }

    fn settled(self) -> bool {
        self.value == self.target()
    }

    /// O valor em `now`, andando em direção ao alvo desde `at`.
    fn advance(self, now: Instant, duration: Duration, reduce: bool) -> Self {
        let target = self.target();
        let value = if reduce { target } else { step_toward(self.value, target, now.saturating_duration_since(self.at), duration) };
        Self { value, at: now, ..self }
    }

    /// O ponteiro entrou ou saiu em `now`: fecha o caminho até aqui com o alvo
    /// antigo e só então troca o alvo. Depois de muito tempo parado o valor já
    /// está no alvo antigo, e a animação começa dali, sem pulo.
    fn hover(self, hovered: bool, now: Instant, duration: Duration, reduce: bool) -> Self {
        Self { hovered, ..self.advance(now, duration, reduce) }
    }
}

type SharedTrack = Rc<Cell<HoverTrack>>;

fn shared_track(window: &mut Window, id: &GlobalElementId) -> SharedTrack {
    window.with_element_state(id, |state: Option<SharedTrack>, _| {
        let track = state.unwrap_or_else(|| Rc::new(Cell::new(HoverTrack::new(Instant::now()))));
        (track.clone(), track)
    })
}

/// No desenho: se o hover mudou sem o mouse se mexer (a lista rolou embaixo
/// do ponteiro), troca o alvo e pede o próximo quadro.
fn sync_hover(track: &SharedTrack, hitbox: &Hitbox, duration: Duration, window: &mut Window, cx: &App) {
    let hovered = hitbox.is_hovered(window);
    let t = track.get();
    if hovered != t.hovered {
        track.set(t.hover(hovered, Instant::now(), duration, cx.reduce_motion()));
        window.request_animation_frame();
    }
}

/// Fora do desenho: ao entrar ou sair, troca o alvo e pede um quadro novo.
fn watch_hover(track: SharedTrack, hitbox: Hitbox, duration: Duration, window: &mut Window) {
    window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let hovered = hitbox.is_hovered(window);
        let t = track.get();
        if hovered != t.hovered {
            track.set(t.hover(hovered, Instant::now(), duration, cx.reduce_motion()));
            window.refresh();
        }
    });
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
        let track = shared_track(window, id.expect("HoverFade tem id"));
        sync_hover(&track, hitbox, HOVER_FADE, window, cx);
        let t = track.get().advance(Instant::now(), HOVER_FADE, cx.reduce_motion());
        track.set(t);
        if !t.settled() {
            window.request_animation_frame();
        }
        let value = t.value;
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
        watch_hover(track, hitbox.clone(), HOVER_FADE, window);
    }
}

// --- Hover animado ---

type BuildFn = Box<dyn FnOnce(f32, &mut Window, &mut App) -> AnyElement>;

/// Elemento cujo conteúdo é montado a cada quadro com o progresso do hover
/// (0 = em repouso, 1 = ponteiro em cima), que anda nos dois sentidos em
/// `duration` e reverte do valor atual. O GPUI não tem transição de CSS: aqui
/// o cartão inteiro pode animar cantos, tamanhos e cores a partir de um número.
///
/// O progresso é linear; quem monta aplica a curva (`motion::ease_out_cubic`).
/// Com "reduzir movimento" pula direto para o fim.
pub struct HoverAnim {
    id: ElementId,
    duration: Duration,
    build: Option<BuildFn>,
    child: Option<AnyElement>,
    track: Option<SharedTrack>,
}

/// `id` precisa ser estável (do domínio, não da posição na lista).
pub fn hover_anim<E: IntoElement>(
    id: impl Into<ElementId>,
    duration: Duration,
    build: impl FnOnce(f32, &mut Window, &mut App) -> E + 'static,
) -> HoverAnim {
    HoverAnim {
        id: id.into(),
        duration,
        build: Some(Box::new(move |t, window, cx| build(t, window, cx).into_any_element())),
        child: None,
        track: None,
    }
}

impl IntoElement for HoverAnim {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for HoverAnim {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(&mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let track = shared_track(window, id.expect("HoverAnim tem id"));
        let t = track.get().advance(Instant::now(), self.duration, cx.reduce_motion());
        track.set(t);
        if !t.settled() {
            window.request_animation_frame();
        }
        self.track = Some(track);
        let value = t.value;
        let build = self.build.take().expect("HoverAnim monta uma vez por quadro");
        let mut child = build(value, window, cx);
        let layout = child.request_layout(window, cx);
        self.child = Some(child);
        (layout, ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Hitbox {
        if let Some(child) = self.child.as_mut() {
            child.prepaint(window, cx);
        }
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), hitbox: &mut Hitbox, window: &mut Window, cx: &mut App) {
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
        if let Some(track) = self.track.take() {
            sync_hover(&track, hitbox, self.duration, window, cx);
            watch_hover(track, hitbox.clone(), self.duration, window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_reaches_target_in_hover_duration() {
        assert_eq!(step_toward(0., 1., HOVER_FADE, HOVER_FADE), 1.);
        assert_eq!(step_toward(1., 0., HOVER_FADE, HOVER_FADE), 0.);
    }

    #[test]
    fn fade_is_gradual_and_reverses_from_current_value() {
        let half = step_toward(0., 1., HOVER_FADE / 2, HOVER_FADE);
        assert!((half - 0.5).abs() < 0.01, "metade do tempo, metade do caminho: {half}");
        // Saiu no meio: começa a apagar de onde estava, não de 1.
        let back = step_toward(half, 0., HOVER_FADE / 4, HOVER_FADE);
        assert!((back - 0.25).abs() < 0.01, "{back}");
    }

    const D: Duration = Duration::from_millis(400);

    #[test]
    fn enter_after_long_idle_starts_from_rest() {
        let t0 = Instant::now();
        let idle = t0 + Duration::from_secs(60);
        // Um minuto parado e o ponteiro entra: começa do 0, não pula para o fim.
        let entered = HoverTrack::new(t0).hover(true, idle, D, false);
        assert_eq!(entered.value, 0.);
        assert!(!entered.settled(), "tem para onde andar: pede quadros");
        let quarter = entered.advance(idle + D / 4, D, false);
        assert!((quarter.value - 0.25).abs() < 0.01, "{quarter:?}");
        assert!(entered.advance(idle + D, D, false).settled());
    }

    #[test]
    fn leave_mid_animation_reverses_from_current_value() {
        let t0 = Instant::now();
        let half = HoverTrack::new(t0).hover(true, t0, D, false).advance(t0 + D / 2, D, false);
        assert!((half.value - 0.5).abs() < 0.01, "{half:?}");
        let left = half.hover(false, t0 + D / 2, D, false);
        assert!((left.value - 0.5).abs() < 0.01, "sair não pula: {left:?}");
        let back = left.advance(t0 + D / 2 + D / 4, D, false);
        assert!((back.value - 0.25).abs() < 0.01, "{back:?}");
        let rest = left.advance(t0 + D * 2, D, false);
        assert_eq!(rest.value, 0.);
        assert!(rest.settled(), "assentado: para de pedir quadros");
    }

    #[test]
    fn reduced_motion_jumps_to_target() {
        let t0 = Instant::now();
        let t = HoverTrack::new(t0).hover(true, t0, D, true).advance(t0, D, true);
        assert_eq!(t.value, 1.);
    }

    #[test]
    fn step_follows_its_own_duration() {
        let d = Duration::from_millis(400);
        assert_eq!(step_toward(0., 1., d, d), 1.);
        let quarter = step_toward(0., 1., d / 4, d);
        assert!((quarter - 0.25).abs() < 0.01, "{quarter}");
        assert_eq!(step_toward(0.6, 0., Duration::ZERO, d), 0.6, "sem tempo, não anda");
        assert_eq!(step_toward(0.3, 1., Duration::from_millis(5), Duration::ZERO), 1., "duração zero pula para o fim");
    }
}
