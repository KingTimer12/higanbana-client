// liquid_glass.wgsl — vidro líquido: refrata, desfoca, tinge e brilha sobre o que está atrás.
//
// Parâmetros (pixels lógicos; o shader multiplica por `p.scale`):
//   param(0) = (refração, raio do desfoque, largura do chanfro, intensidade do brilho 0..1)
//   param(1) = tinta: rgb + força em `a` (0 = vidro incolor)
//
// Ideia: a placa de vidro é plana no meio e curva num chanfro junto da borda. Na curva a luz
// entorta: o ponto lido do backdrop anda para dentro, ao longo da normal da SDF, tanto mais quanto
// mais íngreme a superfície. Cada canal anda um pouco diferente (dispersão), o resultado é
// desfocado, tingido e recebe um reflexo especular que gira com o tempo.

// Distância assinada até a borda arredondada deste elemento (negativa dentro).
fn glass_sdf(p: ShadeInput, px: vec2<f32>) -> f32 {
    return rounded_sdf(px, p.size, p.radius);
}

// Normal da borda (aponta para fora): gradiente da SDF por diferenças centrais.
fn glass_normal(p: ShadeInput) -> vec2<f32> {
    let e = vec2<f32>(1.0, 0.0);
    let g = vec2<f32>(
        glass_sdf(p, p.px + e.xy) - glass_sdf(p, p.px - e.xy),
        glass_sdf(p, p.px + e.yx) - glass_sdf(p, p.px - e.yx),
    );
    let l = length(g);
    return select(vec2<f32>(0.0), g / l, l > 1e-4);
}

// Inclinação da superfície no chanfro: 0 no miolo plano, 1 rente à borda. Perfil de um quarto de
// círculo (a derivada cresce rápido perto da borda, como uma gota).
fn glass_slope(depth: f32, bevel: f32) -> f32 {
    let t = saturate(depth / max(bevel, 1.0)); // 0 na borda, 1 onde o chanfro acaba
    let s = 1.0 - t;
    return s * s * (1.5 - 0.5 * s);
}

// Desfoque mais liso que um `backdrop_blur` só: a grade 5x5 do prelúdio tem passo de meio raio e
// deixa "fantasmas" em traços finos; uma 2ª grade deslocada meio passo na diagonal preenche os vãos.
fn glass_blur(screen: vec2<f32>, blur: f32) -> vec3<f32> {
    let half_step = vec2<f32>(blur * 0.25);
    return (backdrop_blur(screen, blur).rgb + backdrop_blur(screen + half_step, blur).rgb) * 0.5;
}

// Lê o backdrop desfocado num ponto deslocado, com dispersão (as franjas coloridas do vidro): o
// vermelho anda um pouco menos e o azul um pouco mais que o verde. Para não triplicar o desfoque
// (caro na GPU e na compilação: o D3DCompile desenrola cada grade 5x5), a dispersão vem de três
// leituras nítidas e só soma a diferença entre canais; no miolo plano `offset` é zero e ela some.
fn glass_sample(screen: vec2<f32>, offset: vec2<f32>, blur: f32) -> vec3<f32> {
    let dispersion = 0.12;
    let base = glass_blur(screen + offset, blur);
    let sharp = backdrop(screen + offset).rgb;
    let r = backdrop(screen + offset * (1.0 - dispersion)).r - sharp.r;
    let b = backdrop(screen + offset * (1.0 + dispersion)).b - sharp.b;
    return base + vec3<f32>(r, 0.0, b);
}

fn shade(p: ShadeInput) -> vec4<f32> {
    let cfg = param(0u);
    let tint = param(1u);
    let refraction = cfg.x * p.scale;
    let blur = cfg.y * p.scale;
    let bevel = max(cfg.z, 4.0) * p.scale;
    let shine = cfg.w;

    let depth = -glass_sdf(p, p.px); // distância para dentro da borda
    let n = glass_normal(p);
    let slope = glass_slope(depth, bevel);

    // Refração: lê mais para dentro onde o vidro é curvo (efeito de lente na borda). No chanfro o
    // desfoque cai pela metade, para as linhas entortadas pela lente continuarem legíveis.
    let offset = -n * refraction * slope;
    var col = glass_sample(p.screen, offset, blur * (1.0 - 0.5 * slope));

    // Vidro realça um pouco a saturação e o brilho do que está atrás; depois a tinta.
    let luma = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
    col = mix(vec3<f32>(luma), col, 1.2) + vec3<f32>(0.025);
    col = mix(col, tint.rgb, saturate(tint.a));

    // Reflexo especular: uma luz que gira devagar ilumina o lado da borda virado para ela;
    // o lado oposto recebe um rebote mais fraco.
    let angle = p.time * 0.6;
    let light = vec2<f32>(cos(angle), sin(angle));
    let rim = pow(slope, 3.0);
    let facing = dot(n, light);
    let spec = pow(saturate(facing), 8.0) * rim + pow(saturate(-facing), 8.0) * rim * 0.35;

    // Faixa de brilho que varre o vidro na diagonal, como luz passando.
    let sweep = fract(p.time * 0.08) * 3.0 - 1.0;
    let band = p.uv.x * 0.75 + p.uv.y * 0.25 - sweep;
    let sheen = exp(-band * band * 40.0) * 0.08;

    // Filete fino e claro bem na borda: dá o contorno do vidro sobre fundos escuros.
    let edge = 1.0 - smoothstep(0.0, 1.5 * p.scale, depth);

    col += vec3<f32>(spec * 0.9 + sheen + edge * 0.18) * shine;
    return vec4<f32>(saturate(col), 1.0);
}
