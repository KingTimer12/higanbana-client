// prelude.wgsl — vem antes do código do usuário (ver compose.rs). Prefixo `hg_` é reservado.
// Bindings fixos em todos os backends (ver translate.rs para HLSL/GLSL).

// Layout (std140/cbuffer, só vec4): total 320 bytes = HG_SHADER_UNIFORM_BYTES (hgui.h).
struct HgShaderUniforms {
    rect: vec4<f32>,                     // offset   0: x, y, w, h em pixels (origem no topo-esquerda)
    radius: vec4<f32>,                   // offset  16: cantos tl, tr, br, bl em pixels
    time_scale_opacity_flags: vec4<f32>, // offset  32: tempo (s), escala, opacidade, flags (HG_SHADER_* como f32)
    viewport: vec4<f32>,                 // offset  48: largura, altura, 0, sinal do Y do backdrop (< 0 = inverter, OpenGL)
    params: array<vec4<f32>, 16>,        // offset  64: 16 vec4 (stride 16, 256 bytes)
}

@group(0) @binding(0) var<uniform> hg_u: HgShaderUniforms;
@group(0) @binding(1) var hg_backdrop: texture_2d<f32>;
@group(0) @binding(2) var hg_backdrop_sampler: sampler;

struct ShadeInput {
    uv: vec2<f32>,     // 0..1 dentro do retângulo
    px: vec2<f32>,     // posição em pixels dentro do retângulo
    size: vec2<f32>,   // tamanho do retângulo em pixels
    screen: vec2<f32>, // posição em pixels na janela
    time: f32,         // segundos desde a criação do Shader
    scale: f32,        // DPI
    radius: vec4<f32>, // cantos tl, tr, br, bl em pixels
}

// Parâmetro `i` (0..15) do app; índices maiores leem o último.
fn param(i: u32) -> vec4<f32> {
    return hg_u.params[min(i, 15u)];
}

// O que já foi desenhado atrás (bilinear). Transparente sem `.backdrop(true)`.
fn backdrop(screen_px: vec2<f32>) -> vec4<f32> {
    if ((u32(hg_u.time_scale_opacity_flags.w) & 1u) == 0u) {
        return vec4<f32>(0.0);
    }
    var uv = screen_px / hg_u.viewport.xy;
    if (hg_u.viewport.w < 0.0) {
        uv.y = 1.0 - uv.y;
    }
    return textureSampleLevel(hg_backdrop, hg_backdrop_sampler, uv, 0.0);
}

fn hg_binomial(k: i32) -> f32 {
    return select(select(6.0, 4.0, abs(k) == 1), 1.0, abs(k) == 2);
}

// Desfoque do backdrop: 5x5 amostras com pesos binomiais (1 4 6 4 1); raio limitado a 64 px,
// a margem que os backends copiam em volta do retângulo.
fn backdrop_blur(screen_px: vec2<f32>, radius: f32) -> vec4<f32> {
    let r = clamp(radius, 0.0, 64.0);
    if (r < 0.5) {
        return backdrop(screen_px);
    }
    let step = r * 0.5;
    var acc = vec4<f32>(0.0);
    for (var j = -2; j <= 2; j++) {
        for (var i = -2; i <= 2; i++) {
            let o = vec2<f32>(f32(i), f32(j)) * step;
            acc += backdrop(screen_px + o) * (hg_binomial(i) * hg_binomial(j));
        }
    }
    return acc / 256.0;
}

// Distância assinada ao retângulo arredondado (negativa dentro). `px` em 0..size.
fn rounded_sdf(px: vec2<f32>, size: vec2<f32>, radius: vec4<f32>) -> f32 {
    let b = size * 0.5;
    let p = px - b;
    var rr = select(select(radius.z, radius.w, p.x < 0.0), select(radius.y, radius.x, p.x < 0.0), p.y < 0.0);
    rr = min(rr, min(b.x, b.y));
    let q = abs(p) - b + rr;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}

// ---- código do usuário ----
