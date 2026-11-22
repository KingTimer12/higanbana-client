//! Polígono (rasterizado em cache) e imagem com zoom (recorte por uv).
use super::shape::{cover_uv, rasterize};
use super::Canvas;
use crate::layout::tests::build;
use crate::prelude::*;
use crate::{Corners, Point, Quad, Rect, TextStyle};

struct V;

#[test]
fn poligono_menos_de_3_pontos_nao_rasteriza() {
    assert!(rasterize(&[(0.0, 0.0), (1.0, 1.0)], 10, 10).is_none());
    assert!(rasterize(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], 0, 10).is_none());
}

#[test]
fn triangulo_cobre_dentro_e_nao_fora() {
    // Triângulo no canto de cima à direita: (0,0) (1,0) (1,1).
    let img = rasterize(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], 20, 20).unwrap();
    assert!(img.pending_pixel(17, 3).unwrap()[3] > 200);
    assert_eq!(img.pending_pixel(3, 17).unwrap()[3], 0);
}

#[test]
fn zoom_1_e_cover_sem_corte_em_quadrado() {
    assert_eq!(cover_uv((100.0, 100.0), (50, 50), 1.0), Rect::new(0.0, 0.0, 1.0, 1.0));
}

#[test]
fn zoom_2_recorta_o_centro() {
    assert_eq!(cover_uv((100.0, 100.0), (50, 50), 2.0), Rect::new(0.25, 0.25, 0.5, 0.5));
}

#[test]
fn cover_largo_corta_os_lados() {
    assert_eq!(cover_uv((100.0, 100.0), (200, 100), 1.0), Rect::new(0.25, 0.0, 0.5, 1.0));
}

#[derive(Default)]
struct Rec {
    images: Vec<(Rect, Color)>,
    uvs: Vec<(Rect, Rect)>,
}

impl Canvas for Rec {
    fn quad(&mut self, _: &Quad) {}
    fn image(&mut self, _: &ImageSource, r: Rect, _: Corners<f32>, tint: Color) {
        self.images.push((r, tint));
    }
    fn image_uv(&mut self, _: &ImageSource, r: Rect, uv: Rect, _: Corners<f32>, _: Color) {
        self.uvs.push((r, uv));
    }
    fn text(&mut self, _: &str, _: &TextStyle, _: Point, _: Option<f32>, _: Color) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {}
}

#[test]
fn poligono_pinta_tingido_na_propria_caixa() {
    let red = rgb(0xff0000);
    let mut t = build::<V>(div().child(polygon(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], red).w(40.0).h(20.0)), 300.0, 300.0);
    t.resolve_icons(1.0);
    let mut r = Rec::default();
    t.paint(&mut r);
    assert_eq!(r.images, vec![(Rect::new(0.0, 0.0, 40.0, 20.0), red)]);
}

#[test]
fn img_zoom_usa_uv_na_caixa_toda() {
    let src = ImageData::from_rgba8(50, 50, vec![0; 50 * 50 * 4]);
    let t = build::<V>(div().child(img(src).w(100.0).h(100.0).object_fit(crate::style::ObjectFit::Cover).zoom(2.0)), 300.0, 300.0);
    let mut r = Rec::default();
    t.paint(&mut r);
    assert_eq!(r.uvs, vec![(Rect::new(0.0, 0.0, 100.0, 100.0), Rect::new(0.25, 0.25, 0.5, 0.5))]);
}
