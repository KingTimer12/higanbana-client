//! Ícones SVG: rasterização, cache por tamanho físico e tint pela cor do texto.
use super::raster;
use crate::layout::tests::build;
use crate::paint::Canvas;
use crate::prelude::*;
use crate::{Corners, Point, Quad, Rect, TextStyle};

struct V;

#[derive(Default)]
struct Rec {
    images: Vec<(Rect, Color, (u32, u32))>,
}

impl Canvas for Rec {
    fn quad(&mut self, _: &Quad) {}
    fn image(&mut self, src: &ImageSource, r: Rect, _: Corners<f32>, tint: Color) {
        self.images.push((r, tint, src.size()));
    }
    fn text(&mut self, _: &str, _: &TextStyle, _: Point, _: Option<f32>, _: Color) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {}
}

fn alpha(src: &ImageSource, x: u32, y: u32) -> u8 {
    src.pending_pixel(x, y).expect("pixels ainda não enviados")[3]
}

#[test]
fn raster_has_alpha() {
    let src = raster::get("plus", 16).expect("plus existe");
    assert_eq!(src.size(), (16, 16));
    assert!(alpha(&src, 8, 8) > 0, "centro da cruz pintado");
    assert_eq!(alpha(&src, 0, 0), 0, "canto vazio");
}

#[test]
fn icon_cache_reuses() {
    let a = raster::get("play", 32).unwrap();
    let b = raster::get("play", 32).unwrap();
    let c = raster::get("play", 16).unwrap();
    assert_eq!(a.id(), b.id());
    assert_ne!(a.id(), c.id());
}

#[test]
fn unknown_icon_is_empty_box() {
    let mut t = build::<V>(div().child(icon("nao-existe").size(20.0)), 100.0, 100.0);
    assert_eq!(t.rect(1).size, crate::size(20.0, 20.0));
    t.resolve_icons(1.0);
    let mut r = Rec::default();
    t.paint(&mut r);
    assert!(r.images.is_empty());
}

#[test]
fn icon_tinted_by_text_color() {
    let red = rgb(0xff0000);
    let mut t = build::<V>(div().text_color(red).child(icon("play")), 100.0, 100.0);
    t.resolve_icons(2.0);
    let mut r = Rec::default();
    t.paint(&mut r);
    assert_eq!(r.images.len(), 1);
    let (rect, tint, px) = r.images[0];
    assert_eq!(rect.size, crate::size(16.0, 16.0), "tamanho padrão 16");
    assert_eq!(tint, red);
    assert_eq!(px, (32, 32), "rasterizado em pixels físicos");
}

#[test]
fn app_icon_set_registers() {
    const DOT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><rect width="8" height="8"/></svg>"#;
    assert!(raster::get("ponto-app", 8).is_none());
    let _app = crate::App::new().icons(super::IconSet::new().add("ponto-app", DOT));
    let src = raster::get("ponto-app", 8).expect("registrado");
    assert_eq!(alpha(&src, 4, 4), 255);
}
