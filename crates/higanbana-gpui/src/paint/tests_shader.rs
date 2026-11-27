//! Shader como fundo de div: ordem sombra → shader → borda → filhos, opacidade e magenta.
use super::Canvas;
use crate::gfx::{ShaderError, ShaderFill};
use crate::layout::tests::build;
use crate::prelude::*;
use crate::{Corners, Point, Quad, Rect, TextStyle};

const RED: Color = rgb(0xff0000);
const MAGENTA: Color = rgb(0xff00ff);
const SRC: &str = "fn shade(p: ShadeInput) -> vec4<f32> { return vec4(0.0, 1.0, 0.0, 1.0); }";

#[derive(Debug, PartialEq)]
enum Op {
    Quad(Quad),
    Shader(Rect, Corners<f32>, f32),
    Text(String),
    Layer,
}

#[derive(Default)]
struct Rec(Vec<Op>);

impl Canvas for Rec {
    fn quad(&mut self, q: &Quad) {
        self.0.push(Op::Quad(*q));
    }
    fn image(&mut self, _: &ImageSource, _: Rect, _: Corners<f32>, _: Color) {}
    fn text(&mut self, t: &str, _: &TextStyle, _: Point, _: Option<f32>, _: Color) {
        self.0.push(Op::Text(t.into()));
    }
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn layer(&mut self) {
        self.0.push(Op::Layer);
    }
    fn shader(&mut self, rect: Rect, radius: Corners<f32>, _: &ShaderFill, opacity: f32) {
        self.0.push(Op::Shader(rect, radius, opacity));
    }
}

struct V;

/// Sem a feature `shaders` não há como criar um `Shader`: os testes viram no-op.
fn shader() -> Option<Shader> {
    Shader::wgsl(SRC).ok()
}

fn paint(root: Element<V>) -> Vec<Op> {
    let t = build::<V>(div().child(root), 200.0, 200.0);
    let mut rec = Rec::default();
    t.paint(&mut rec);
    rec.0
}

#[test]
fn shader_substitui_o_fundo() {
    let Some(s) = shader() else { return };
    let ops = paint(div().w(100.0).h(50.0).bg(RED).shader(&s, params![[1., 0., 0., 0.]]));
    assert!(!ops.iter().any(|o| matches!(o, Op::Quad(q) if q.fill == RED)), "fundo vermelho não pinta: {ops:?}");
    assert_eq!(ops, vec![Op::Shader(Rect::new(0.0, 0.0, 100.0, 50.0), Corners::all(0.0), 1.0)]);
}

#[test]
fn shader_entre_sombra_e_borda() {
    let Some(s) = shader() else { return };
    let el = div().w(100.0).h(50.0).rounded(8.0).shadow(Color::BLACK, 6.0).border(2.0, RED).shader(&s, params![]).child(text("oi"));
    let ops = paint(el);
    let rect = Rect::new(0.0, 0.0, 100.0, 50.0);
    assert_eq!(ops.len(), 4, "{ops:?}");
    let Op::Quad(sombra) = &ops[0] else { panic!("1º é a sombra: {ops:?}") };
    assert_eq!((sombra.fill, sombra.border_width, sombra.shadow_blur), (Color::TRANSPARENT, 0.0, 6.0));
    assert_eq!(ops[1], Op::Shader(rect, Corners::all(8.0), 1.0));
    let Op::Quad(borda) = &ops[2] else { panic!("3º é a borda: {ops:?}") };
    assert_eq!((borda.fill, borda.border, borda.border_width, borda.shadow_blur), (Color::TRANSPARENT, RED, 2.0, 0.0));
    assert_eq!(borda.radius, Corners::all(8.0));
    assert_eq!(ops[3], Op::Text("oi".into()), "filhos por cima");
}

#[test]
fn shader_herda_opacidade() {
    let Some(s) = shader() else { return };
    let ops = paint(div().opacity(0.5).child(div().w(10.0).h(10.0).shader(&s, params![])));
    assert!(matches!(ops.as_slice(), [Op::Shader(_, _, o)] if *o == 0.5), "{ops:?}");
}

#[test]
fn shader_com_erro_pinta_magenta() {
    let Some(s) = shader() else { return };
    assert!(s.set_error(ShaderError::Native("falhou".into())));
    assert!(!s.set_error(ShaderError::Native("falhou".into())), "mesmo erro não é novo");
    let ops = paint(div().w(30.0).h(20.0).rounded(4.0).shader(&s, params![]));
    let [Op::Quad(q)] = ops.as_slice() else { panic!("{ops:?}") };
    assert_eq!((q.rect, q.fill, q.radius), (Rect::new(0.0, 0.0, 30.0, 20.0), MAGENTA, Corners::all(4.0)));
    assert_eq!(s.last_error(), Some(ShaderError::Native("falhou".into())));
}

#[test]
fn texto_depois_do_shader_nao_abre_camada_extra() {
    // O core já fecha a camada no shader; um quad depois de texto pintado antes não colide.
    let Some(s) = shader() else { return };
    let el = div().child(text("antes")).child(div().w(50.0).h(20.0).shader(&s, params![])).child(div().absolute().top(0.0).left(0.0).w(50.0).h(20.0).bg(RED));
    let ops = paint(el);
    assert!(!ops.contains(&Op::Layer), "{ops:?}");
}
