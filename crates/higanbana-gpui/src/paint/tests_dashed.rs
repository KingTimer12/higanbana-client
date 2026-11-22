//! Borda tracejada: traços de 4 px com vão de 4 px.
use super::painter::{dash_quads, dash_segments};
use crate::color::Color;
use crate::geometry::Rect;

#[test]
fn borda_tracejada_tem_vaos() {
    let segs = dash_segments(40.0);
    assert_eq!(segs.len(), 5);
    assert_eq!(segs[1], (8.0, 4.0));
    assert_eq!(dash_segments(10.0).last(), Some(&(8.0, 2.0)), "último traço cortado no fim");
}

#[test]
fn linha_tracejada_so_nos_lados_com_comprimento() {
    // Linha de 40×1 com borda 1: topo e base tracejados, laterais de 1 px viram um traço só.
    let q = dash_quads(Rect::new(0.0, 0.0, 40.0, 1.0), 1.0, Color::BLACK);
    assert_eq!(q.len(), 5 + 5 + 1 + 1);
}
