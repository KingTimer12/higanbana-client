//! Métodos fluentes estilo GPUI (`.flex().flex_col().gap(8.).p(16.)`).
use super::*;
use crate::color::Color;
use crate::geometry::Corners;

/// Tudo que tem um [`Style`] mutável ganha os construtores fluentes.
pub trait Styled: Sized {
    fn style_mut(&mut self) -> &mut Style;

    fn with(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(self.style_mut());
        self
    }

    // ---- display / direção ----
    fn flex(self) -> Self { self.with(|s| s.display = Display::Flex) }
    fn hidden(self) -> Self { self.with(|s| s.display = Display::None) }
    fn flex_row(self) -> Self { self.with(|s| s.direction = Direction::Row) }
    fn flex_col(self) -> Self { self.with(|s| s.direction = Direction::Column) }
    fn flex_row_reverse(self) -> Self { self.with(|s| s.direction = Direction::RowReverse) }
    fn flex_col_reverse(self) -> Self { self.with(|s| s.direction = Direction::ColumnReverse) }
    fn flex_wrap(self) -> Self { self.with(|s| s.wrap = true) }

    // ---- flex item ----
    fn flex_grow(self) -> Self { self.with(|s| s.flex_grow = 1.0) }
    fn flex_shrink_0(self) -> Self { self.with(|s| s.flex_shrink = 0.0) }
    /// grow 1, shrink 1, basis 0: divide o espaço por igual.
    fn flex_1(self) -> Self {
        self.with(|s| { s.flex_grow = 1.0; s.flex_shrink = 1.0; s.flex_basis = Length::Px(0.0) })
    }
    fn flex_none(self) -> Self { self.with(|s| { s.flex_grow = 0.0; s.flex_shrink = 0.0 }) }
    fn basis(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.flex_basis = l) }

    // ---- espaçamento ----
    fn gap(self, v: f32) -> Self { self.with(|s| { s.gap_x = v; s.gap_y = v }) }
    fn gap_x(self, v: f32) -> Self { self.with(|s| s.gap_x = v) }
    fn gap_y(self, v: f32) -> Self { self.with(|s| s.gap_y = v) }
    fn p(self, v: f32) -> Self { self.with(|s| s.padding = Edges::all(v)) }
    fn px(self, v: f32) -> Self { self.with(|s| { s.padding.left = v; s.padding.right = v }) }
    fn py(self, v: f32) -> Self { self.with(|s| { s.padding.top = v; s.padding.bottom = v }) }
    fn pt(self, v: f32) -> Self { self.with(|s| s.padding.top = v) }
    fn pr(self, v: f32) -> Self { self.with(|s| s.padding.right = v) }
    fn pb(self, v: f32) -> Self { self.with(|s| s.padding.bottom = v) }
    fn pl(self, v: f32) -> Self { self.with(|s| s.padding.left = v) }
    fn m(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| s.margin = Edges::all(v)) }
    fn mx(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| { s.margin.left = v; s.margin.right = v }) }
    fn my(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| { s.margin.top = v; s.margin.bottom = v }) }
    fn mt(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| s.margin.top = v) }
    fn mr(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| s.margin.right = v) }
    fn mb(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| s.margin.bottom = v) }
    fn ml(self, v: impl Into<Length>) -> Self { let v = v.into(); self.with(|s| s.margin.left = v) }

    // ---- tamanho ----
    fn w(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.width = l) }
    fn h(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.height = l) }
    fn size(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| { s.width = l; s.height = l }) }
    fn w_full(self) -> Self { self.w(pct(100.0)) }
    fn h_full(self) -> Self { self.h(pct(100.0)) }
    fn size_full(self) -> Self { self.w_full().h_full() }
    fn min_w(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.min_width = l) }
    fn min_h(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.min_height = l) }
    fn max_w(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.max_width = l) }
    fn max_h(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.max_height = l) }

    // ---- alinhamento ----
    fn items_start(self) -> Self { self.with(|s| s.align_items = Some(Align::Start)) }
    fn items_center(self) -> Self { self.with(|s| s.align_items = Some(Align::Center)) }
    fn items_end(self) -> Self { self.with(|s| s.align_items = Some(Align::End)) }
    fn items_stretch(self) -> Self { self.with(|s| s.align_items = Some(Align::Stretch)) }
    fn items_baseline(self) -> Self { self.with(|s| s.align_items = Some(Align::Baseline)) }
    fn self_start(self) -> Self { self.with(|s| s.align_self = Some(Align::Start)) }
    fn self_center(self) -> Self { self.with(|s| s.align_self = Some(Align::Center)) }
    fn self_end(self) -> Self { self.with(|s| s.align_self = Some(Align::End)) }
    fn justify_start(self) -> Self { self.with(|s| s.justify = Some(Justify::Start)) }
    fn justify_center(self) -> Self { self.with(|s| s.justify = Some(Justify::Center)) }
    fn justify_end(self) -> Self { self.with(|s| s.justify = Some(Justify::End)) }
    fn justify_between(self) -> Self { self.with(|s| s.justify = Some(Justify::SpaceBetween)) }
    fn justify_around(self) -> Self { self.with(|s| s.justify = Some(Justify::SpaceAround)) }
    fn justify_evenly(self) -> Self { self.with(|s| s.justify = Some(Justify::SpaceEvenly)) }

    // ---- posição ----
    fn relative(self) -> Self { self.with(|s| s.position = Position::Relative) }
    fn absolute(self) -> Self { self.with(|s| s.position = Position::Absolute) }
    fn top(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.inset.top = l) }
    fn right(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.inset.right = l) }
    fn bottom(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.inset.bottom = l) }
    fn left(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.inset.left = l) }
    fn inset(self, l: impl Into<Length>) -> Self { let l = l.into(); self.with(|s| s.inset = Edges::all(l)) }
    fn overflow_hidden(self) -> Self { self.with(|s| s.overflow = Overflow::Hidden) }

    // ---- visual ----
    fn bg(self, c: Color) -> Self { self.with(|s| s.background = Some(c)) }
    fn border(self, width: f32, c: Color) -> Self { self.with(|s| { s.border_width = width; s.border_color = c }) }
    fn border_color(self, c: Color) -> Self { self.with(|s| s.border_color = c) }
    fn rounded(self, r: f32) -> Self { self.with(|s| s.radius = Corners::all(r)) }
    fn rounded_corners(self, r: Corners<f32>) -> Self { self.with(|s| s.radius = r) }
    fn rounded_full(self) -> Self { self.rounded(9999.0) }
    fn shadow(self, color: Color, blur: f32) -> Self { self.with(|s| s.shadow = Some(Shadow { color, blur })) }
    fn opacity(self, v: f32) -> Self { self.with(|s| s.opacity = v.clamp(0.0, 1.0)) }
    /// Desloca a subárvore só na pintura e no hit test (o layout não muda).
    fn translate(self, x: f32, y: f32) -> Self { self.with(|s| s.translate = crate::geometry::Point { x, y }) }
    fn translate_x(self, x: f32) -> Self { self.with(|s| s.translate.x = x) }
    fn translate_y(self, y: f32) -> Self { self.with(|s| s.translate.y = y) }
    /// Escala a subárvore em torno do centro (texto, raios e bordas acompanham).
    /// Só vale no estilo base: em `.hover(..)`/foco é ignorado (anime com `cx.animate`).
    fn scale(self, v: f32) -> Self { self.with(|s| s.scale = v.max(0.0)) }
    /// Área que arrasta a janela (barra customizada). Filhos clicáveis continuam clicáveis.
    fn window_drag(self) -> Self { self.window_control(crate::gfx::HitKind::Caption) }
    /// Papel do elemento para o sistema (ex.: `HitKind::Maximize` habilita snap layouts).
    fn window_control(self, k: crate::gfx::HitKind) -> Self { self.with(|s| s.window_hit = Some(k)) }
    /// Cursor do mouse enquanto sobre o elemento.
    fn cursor(self, c: crate::gfx::CursorStyle) -> Self { self.with(|s| s.cursor = Some(c)) }
    fn cursor_pointer(self) -> Self { self.cursor(crate::gfx::CursorStyle::Pointer) }

    // ---- texto (herdado) ----
    fn text_color(self, c: Color) -> Self { self.with(|s| s.text.color = Some(c)) }
    fn text_size(self, v: f32) -> Self { self.with(|s| s.text.size = Some(v)) }
    fn font_weight(self, w: u16) -> Self { self.with(|s| s.text.weight = Some(w)) }
    fn font_bold(self) -> Self { self.font_weight(700) }
    fn italic(self) -> Self { self.with(|s| s.text.italic = Some(true)) }
    fn font_family(self, f: &str) -> Self { let f: std::rc::Rc<str> = f.into(); self.with(|s| s.text.family = Some(f)) }
    fn whitespace_nowrap(self) -> Self { self.with(|s| s.text.wrap = Some(false)) }
}

impl Styled for Style {
    fn style_mut(&mut self) -> &mut Style {
        self
    }
}
