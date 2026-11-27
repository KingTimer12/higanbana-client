//! Shaders do usuário desenhados de verdade em cada backend: janela 320x240 + leitura de pixel.
//!
//! Para ligar um backend novo basta acrescentar uma linha em `backend_tests!` no fim do arquivo:
//! cada caso vira o teste `<backend>::<caso>` (ex.: `cargo test --test shader_backends d3d11`).
//! Coordenadas são pixels FÍSICOS (como `ShaderDraw::rect` e os quads da `DrawList`).
use higanbana_gpui::gfx::shader::Translations;
use higanbana_gpui::prelude::*;
use higanbana_gpui::{params, size, Context, Corners, DrawList, Quad, Rect, Shader, ShaderDraw, Window};

pub type Backend = BackendKind;

const BLACK: u32 = 0x000000;
const RED: u32 = 0xff0000;
const CYAN: u32 = 0x00ffff;
const WHITE: u32 = 0xffffff;
const BLUE: u32 = 0x0000ff;
const YELLOW: u32 = 0xffff00;

/// Uma operação na draw list, na ordem de desenho.
#[allow(clippy::large_enum_variant)] // só em teste
#[derive(Clone)]
enum Op {
    Quad(Rect, Color),
    Shader(ShaderDraw),
}

/// De onde vem o shader do caso.
#[derive(Clone)]
enum Code {
    /// WGSL do usuário (traduzido pelo naga).
    Wgsl(&'static str),
    /// Fontes nativos inválidos em todos os alvos: o compilador do backend tem de recusar.
    Broken,
}

/// Um cenário: o que desenhar, onde ler e o que mais verificar.
#[derive(Clone)]
struct Case {
    code: Code,
    backdrop: bool,
    ops: Vec<Op>,
    points: Vec<(u32, u32)>,
    /// Desenha uma vez, redimensiona para este tamanho físico e lê no 2º quadro.
    resize: Option<(u32, u32)>,
    /// Pontos lidos só no quadro após o resize (depois de `points`): fora da área original.
    resize_points: Vec<(u32, u32)>,
    /// `native_error()` deve vir preenchido (true) ou vazio (false).
    expect_native_error: bool,
}

impl Case {
    fn new(code: Code, ops: Vec<Op>, points: Vec<(u32, u32)>) -> Self {
        Case { code, backdrop: false, ops, points, resize: None, resize_points: Vec::new(), expect_native_error: false }
    }

    fn backdrop(mut self) -> Self {
        self.backdrop = true;
        self
    }
}

fn broken() -> Translations {
    let junk = "isto não é um shader {".to_owned();
    Translations { hlsl: junk.clone(), glsl_vs: junk.clone(), glsl_fs: junk.clone(), spirv: vec![0xdead_beef, 1, 2, 3], wgsl: junk }
}

/// Um caso por vez no processo. Contextos de backends diferentes (D3D11 e D3D12) em threads paralelas
/// derrubam o processo de teste (access violation, já sem os shaders do D3D12); estes testes verificam o
/// desenho de cada backend, não essa concorrência, que fica investigada à parte.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Roda `case` no backend `kind`. `None` = backend indisponível nesta máquina (o teste pula).
/// Devolve os pixels (0x00RRGGBB) nos `points` do último quadro.
fn run_case(kind: Backend, case: Case) -> Option<Vec<u32>> {
    // Um assert que falhou em outro caso envenena o lock; o estado protegido é vazio, segue.
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = Context::new(kind).ok()?;
    if ctx.backend() != kind {
        return None; // o core caiu para outro backend
    }
    let opts = WindowOptions { title: "shader".into(), size: size(320.0, 240.0), resizable: false, ..Default::default() };
    let win = Window::new(&ctx, &opts).expect("janela");
    let gpu = match &case.code {
        Code::Wgsl(src) => ctx.shader(&Shader::wgsl(src).expect("wgsl").backdrop(case.backdrop)),
        Code::Broken => ctx.shader_from_translations(&broken(), case.backdrop),
    }
    .expect("shader");
    let mut list = DrawList::new();
    for op in &case.ops {
        match op {
            Op::Quad(rect, fill) => list.quad(&Quad { rect: *rect, fill: *fill, ..Default::default() }).expect("quad"),
            Op::Shader(d) => list.shader(&gpu, d).expect("draw shader"),
        }
    }
    let mut px = win.present_and_read(Color::BLACK, &list, &case.points).expect("quadro");
    if let Some((w, h)) = case.resize {
        resize_native(&win, w, h);
        win.inject_resize(w, h);
        let points: Vec<_> = case.points.iter().chain(&case.resize_points).copied().collect();
        px = win.present_and_read(Color::BLACK, &list, &points).expect("quadro após resize");
    }
    assert_eq!(gpu.native_error().is_some(), case.expect_native_error, "{kind:?}: native_error = {:?}", gpu.native_error());
    Some(px)
}

/// Redimensiona a HWND de verdade para a área cliente `w`x`h` (físico). O OpenGL desenha no
/// framebuffer padrão, que segue a janela real: só o `inject_resize` (tamanho lógico) deixaria o
/// back buffer em 320x240. Nos D3D o WM_SIZE resultante leva ao mesmo resize do swapchain.
fn resize_native(win: &Window<'_>, w: u32, h: u32) {
    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: isize, rect: *mut WinRect) -> i32;
        fn GetClientRect(hwnd: isize, rect: *mut WinRect) -> i32;
        fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    }
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    let hwnd = win.native_handle() as isize;
    let (mut outer, mut client) = (WinRect::default(), WinRect::default());
    unsafe {
        assert!(GetWindowRect(hwnd, &mut outer) != 0 && GetClientRect(hwnd, &mut client) != 0, "rects da janela");
        // Borda/título = diferença entre a janela e a área cliente (vale para qualquer estilo).
        let cx = w as i32 + (outer.right - outer.left) - client.right;
        let cy = h as i32 + (outer.bottom - outer.top) - client.bottom;
        assert!(SetWindowPos(hwnd, 0, 0, 0, cx, cy, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) != 0, "SetWindowPos");
    }
}

fn skip(kind: Backend) {
    eprintln!("{kind:?} indisponível: caso pulado");
}

/// Cada canal dentro de `tol` do esperado.
fn assert_near(got: u32, want: u32, tol: u32, what: &str) {
    let ch = |v: u32, s: u32| (v >> s) & 0xff;
    let ok = [16, 8, 0].iter().all(|&s| ch(got, s).abs_diff(ch(want, s)) <= tol);
    assert!(ok, "{what}: {got:06x} != {want:06x} (±{tol})");
}

fn shader_at(rect: Rect) -> ShaderDraw {
    ShaderDraw { rect, ..Default::default() }
}

const SOLID_RED: &str = "fn shade(p: ShadeInput) -> vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }";
const HALF_GREEN: &str = "fn shade(p: ShadeInput) -> vec4<f32> { return vec4<f32>(0.0, 1.0, 0.0, 0.5); }";
const SOLID_WHITE: &str = "fn shade(p: ShadeInput) -> vec4<f32> { return vec4<f32>(1.0); }";
const PARAM0: &str = "fn shade(p: ShadeInput) -> vec4<f32> { return param(0u); }";
/// Inverte o que está atrás (alfa 1: `vec4(1.0) - backdrop` zeraria o alfa de um fundo opaco).
const INVERT: &str =
    "fn shade(p: ShadeInput) -> vec4<f32> { let b = backdrop(p.screen); return vec4<f32>(vec3<f32>(1.0) - b.rgb, 1.0); }";

const BOX: Rect = Rect::new(40.0, 40.0, 200.0, 160.0);

fn case_solid(kind: Backend) {
    let case = Case::new(Code::Wgsl(SOLID_RED), vec![Op::Shader(shader_at(BOX))], vec![(140, 120)]);
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![RED]);
}

fn case_corners(kind: Backend) {
    let d = ShaderDraw { radius: Corners::all(40.0), ..shader_at(BOX) };
    let case = Case::new(Code::Wgsl(SOLID_RED), vec![Op::Shader(d)], vec![(41, 41), (140, 120)]);
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![BLACK, RED]);
}

fn case_order(kind: Backend) {
    let area = Rect::new(20.0, 20.0, 280.0, 200.0);
    let ops = vec![
        Op::Quad(area, rgb(0x0000ff)),
        Op::Shader(shader_at(area)),
        Op::Quad(Rect::new(120.0, 100.0, 80.0, 40.0), rgb(0xffffff)),
    ];
    let case = Case::new(Code::Wgsl(HALF_GREEN), ops, vec![(160, 120), (30, 30)]);
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px[0], WHITE, "centro: quad branco por cima do shader");
    assert_near(px[1], 0x008080, 2, "borda: verde meio transparente sobre azul");
}

fn red_backdrop_ops(glass: Rect) -> Vec<Op> {
    vec![Op::Quad(Rect::new(0.0, 0.0, 320.0, 240.0), rgb(0xff0000)), Op::Shader(shader_at(glass))]
}

fn case_backdrop(kind: Backend) {
    let case = Case::new(Code::Wgsl(INVERT), red_backdrop_ops(BOX), vec![(140, 120), (20, 20)]).backdrop();
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![CYAN, RED]);
}

fn case_backdrop_asymmetric(kind: Backend) {
    // Fundo vermelho em cima (y < 50) e azul embaixo; vidro baixo e fora do centro (a margem de 64 px
    // não cobre a janela toda). Um backdrop lido ou copiado de cabeça para baixo (origem embaixo do GL)
    // troca as cores ou lê linhas que nunca foram copiadas.
    let ops = vec![
        Op::Quad(Rect::new(0.0, 0.0, 320.0, 50.0), rgb(0xff0000)),
        Op::Quad(Rect::new(0.0, 50.0, 320.0, 190.0), rgb(0x0000ff)),
        Op::Shader(shader_at(Rect::new(40.0, 20.0, 200.0, 60.0))),
    ];
    let points = vec![(140, 30), (140, 70), (20, 10), (20, 100)];
    let case = Case::new(Code::Wgsl(INVERT), ops, points).backdrop();
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![CYAN, YELLOW, RED, BLUE], "vermelho em cima vira ciano, azul embaixo vira amarelo");
}

fn case_backdrop_off(kind: Backend) {
    let case = Case::new(Code::Wgsl(INVERT), red_backdrop_ops(BOX), vec![(140, 120)]);
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![WHITE], "backdrop() transparente sem .backdrop(true)");
}

fn case_opacity(kind: Backend) {
    let d = ShaderDraw { opacity: 0.5, ..shader_at(BOX) };
    let case = Case::new(Code::Wgsl(SOLID_WHITE), vec![Op::Shader(d)], vec![(140, 120)]);
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_near(px[0], 0x808080, 2, "opacidade 0.5 sobre preto");
}

fn case_params(kind: Backend) {
    let color = [0x33 as f32 / 255.0, 0x66 as f32 / 255.0, 0x99 as f32 / 255.0, 1.0];
    let d = ShaderDraw { params: params![color], ..shader_at(BOX) };
    let case = Case::new(Code::Wgsl(PARAM0), vec![Op::Shader(d)], vec![(140, 120)]);
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_near(px[0], 0x336699, 1, "param(0) como cor");
}

fn case_native_error(kind: Backend) {
    let mut case = Case::new(Code::Broken, vec![Op::Shader(shader_at(BOX))], vec![(140, 120)]);
    case.expect_native_error = true;
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![BLACK], "shader que não compila não desenha");
}

fn case_edge_and_resize(kind: Backend) {
    // Fundo vermelho já no tamanho pós-resize; o 2º vidro fica fora da janela de 320x240 (backdrop vazio,
    // nada a copiar) e só aparece depois do resize, onde um backdrop velho de 320x240 não chega.
    let ops = vec![
        Op::Quad(Rect::new(0.0, 0.0, 640.0, 480.0), rgb(0xff0000)),
        Op::Shader(shader_at(Rect::new(-50.0, -50.0, 200.0, 200.0))),
        Op::Shader(shader_at(Rect::new(400.0, 300.0, 160.0, 120.0))),
    ];
    let mut case = Case::new(Code::Wgsl(INVERT), ops, vec![(10, 10), (200, 200)]).backdrop();
    let Some(px) = run_case(kind, case.clone()) else { return skip(kind) };
    assert_eq!(px, vec![CYAN, RED], "vidro saindo pela borda");
    case.resize = Some((640, 480));
    case.resize_points = vec![(480, 360), (620, 460)];
    let px = run_case(kind, case).expect("backend sumiu entre casos");
    assert_eq!(px, vec![CYAN, RED, CYAN, RED], "vidros após resize para 640x480 (o 2º fora da área antiga)");
}

fn case_overlap(kind: Backend) {
    let ops = vec![
        Op::Quad(Rect::new(0.0, 0.0, 320.0, 240.0), rgb(0xff0000)),
        Op::Shader(shader_at(Rect::new(40.0, 40.0, 160.0, 120.0))),
        Op::Shader(shader_at(Rect::new(120.0, 80.0, 160.0, 120.0))),
    ];
    let case = Case::new(Code::Wgsl(INVERT), ops, vec![(60, 60), (150, 110), (250, 180)]).backdrop();
    let Some(px) = run_case(kind, case) else { return skip(kind) };
    assert_eq!(px, vec![CYAN, RED, CYAN], "o de cima inverte o resultado do de baixo");
}

macro_rules! backend_tests {
    ($($name:ident => $kind:expr),* $(,)?) => {$(
        mod $name {
            use super::*;
            #[test] fn solido() { case_solid($kind) }
            #[test] fn cantos() { case_corners($kind) }
            #[test] fn ordem() { case_order($kind) }
            #[test] fn backdrop() { case_backdrop($kind) }
            #[test] fn backdrop_assimetrico() { case_backdrop_asymmetric($kind) }
            #[test] fn backdrop_desligado() { case_backdrop_off($kind) }
            #[test] fn opacidade() { case_opacity($kind) }
            #[test] fn params() { case_params($kind) }
            #[test] fn erro_nativo() { case_native_error($kind) }
            #[test] fn borda_e_resize() { case_edge_and_resize($kind) }
            #[test] fn sobreposicao() { case_overlap($kind) }
        }
    )*};
}

// Tasks 6-7: acrescentar `vulkan => ...`, `wgpu => ...`.
backend_tests! {
    d3d11 => BackendKind::D3D11,
    d3d12 => BackendKind::D3D12,
    opengl => BackendKind::OpenGL,
}
