//! Conversão do nosso `Style` para `taffy::Style`.
use crate::style::{Align, Direction, Display, Justify, Length, Overflow, Position, Style};
use taffy::style as ts;

fn dim(l: Length) -> ts::Dimension {
    match l {
        Length::Auto => ts::Dimension::auto(),
        Length::Px(v) => ts::Dimension::length(v),
        Length::Percent(v) => ts::Dimension::percent(v / 100.0),
    }
}

fn lpa(l: Length) -> ts::LengthPercentageAuto {
    match l {
        Length::Auto => ts::LengthPercentageAuto::auto(),
        Length::Px(v) => ts::LengthPercentageAuto::length(v),
        Length::Percent(v) => ts::LengthPercentageAuto::percent(v / 100.0),
    }
}

fn lp(v: f32) -> ts::LengthPercentage {
    ts::LengthPercentage::length(v)
}

fn align(a: Align) -> ts::AlignItems {
    match a {
        Align::Start => ts::AlignItems::FLEX_START,
        Align::End => ts::AlignItems::FLEX_END,
        Align::Center => ts::AlignItems::CENTER,
        Align::Stretch => ts::AlignItems::STRETCH,
        Align::Baseline => ts::AlignItems::BASELINE,
    }
}

fn justify(j: Justify) -> ts::JustifyContent {
    match j {
        Justify::Start => ts::JustifyContent::FLEX_START,
        Justify::End => ts::JustifyContent::FLEX_END,
        Justify::Center => ts::JustifyContent::CENTER,
        Justify::SpaceBetween => ts::JustifyContent::SPACE_BETWEEN,
        Justify::SpaceAround => ts::JustifyContent::SPACE_AROUND,
        Justify::SpaceEvenly => ts::JustifyContent::SPACE_EVENLY,
    }
}

/// Mapeia o estilo; `scrollbar_width` fica 0 (barra é desenhada por cima).
pub(crate) fn to_taffy(s: &Style) -> ts::Style {
    let overflow = match s.overflow {
        Overflow::Visible => ts::Overflow::Visible,
        Overflow::Hidden => ts::Overflow::Hidden,
        Overflow::Scroll | Overflow::ScrollX => ts::Overflow::Scroll,
    };
    // Só o eixo que rola fica "Scroll"; o outro recorta.
    let (ox, oy) = match s.overflow {
        Overflow::Scroll => (ts::Overflow::Hidden, overflow),
        Overflow::ScrollX => (overflow, ts::Overflow::Hidden),
        _ => (overflow, overflow),
    };
    ts::Style {
        display: match s.display {
            Display::Flex => ts::Display::Flex,
            Display::None => ts::Display::None,
        },
        flex_direction: match s.direction {
            Direction::Row => ts::FlexDirection::Row,
            Direction::Column => ts::FlexDirection::Column,
            Direction::RowReverse => ts::FlexDirection::RowReverse,
            Direction::ColumnReverse => ts::FlexDirection::ColumnReverse,
        },
        flex_wrap: if s.wrap { ts::FlexWrap::Wrap } else { ts::FlexWrap::NoWrap },
        gap: taffy::Size { width: lp(s.gap_x), height: lp(s.gap_y) },
        padding: taffy::Rect {
            left: lp(s.padding.left),
            right: lp(s.padding.right),
            top: lp(s.padding.top),
            bottom: lp(s.padding.bottom),
        },
        border: taffy::Rect::length(s.border_width),
        margin: taffy::Rect {
            left: lpa(s.margin.left),
            right: lpa(s.margin.right),
            top: lpa(s.margin.top),
            bottom: lpa(s.margin.bottom),
        },
        size: taffy::Size { width: dim(s.width), height: dim(s.height) },
        min_size: taffy::Size { width: lpa(s.min_width), height: lpa(s.min_height) },
        max_size: taffy::Size { width: lpa(s.max_width), height: lpa(s.max_height) },
        flex_grow: s.flex_grow,
        flex_shrink: s.flex_shrink,
        flex_basis: dim(s.flex_basis),
        align_items: s.align_items.map(align).unwrap_or(ts::AlignItems::NORMAL),
        align_self: s.align_self.map(align),
        justify_content: s.justify.map(justify).unwrap_or(ts::JustifyContent::NORMAL),
        position: match s.position {
            Position::Relative => ts::Position::Relative,
            Position::Absolute => ts::Position::Absolute,
        },
        inset: taffy::Rect {
            left: lpa(s.inset.left),
            right: lpa(s.inset.right),
            top: lpa(s.inset.top),
            bottom: lpa(s.inset.bottom),
        },
        overflow: taffy::Point { x: ox, y: oy },
        scrollbar_width: 0.0,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{pct, px, Styled};

    #[test]
    fn maps_flex_spacing_and_sizes() {
        let s = Style::default().flex_col().gap(8.0).p(16.0).w(px(200.0)).h(pct(50.0)).items_center().border(2.0, crate::Color::BLACK);
        let t = to_taffy(&s);
        assert_eq!(t.flex_direction, ts::FlexDirection::Column);
        assert_eq!(t.gap.width, lp(8.0));
        assert_eq!(t.padding.left, lp(16.0));
        assert_eq!(t.size.width, ts::Dimension::length(200.0));
        assert_eq!(t.size.height, ts::Dimension::percent(0.5));
        assert_eq!(t.align_items, ts::AlignItems::CENTER);
        assert_eq!(t.border.top, lp(2.0));
    }

    #[test]
    fn maps_absolute_and_overflow() {
        let s = Style::default().absolute().top(px(4.0)).left(pct(10.0)).overflow_hidden().hidden();
        let t = to_taffy(&s);
        assert_eq!(t.position, ts::Position::Absolute);
        assert_eq!(t.inset.top, ts::LengthPercentageAuto::length(4.0));
        assert_eq!(t.inset.left, ts::LengthPercentageAuto::percent(0.1));
        assert_eq!(t.inset.right, ts::LengthPercentageAuto::auto());
        assert_eq!(t.overflow.y, ts::Overflow::Hidden);
        assert_eq!(t.display, ts::Display::None);
    }
}
