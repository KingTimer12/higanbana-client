// glass_aurora.wgsl — fundo da seção do vidro: manchas de cor que vagueiam devagar.
// Sem backdrop; só serve para o vidro ter algo colorido e em movimento para refratar.

// Mancha gaussiana de raio `r` em volta de `c` (coordenadas com a proporção corrigida).
fn aurora_blob(uv: vec2<f32>, c: vec2<f32>, r: f32) -> f32 {
    let d = uv - c;
    return exp(-dot(d, d) / (r * r));
}

fn shade(p: ShadeInput) -> vec4<f32> {
    let t = p.time * 0.3;
    let aspect = p.size.x / max(p.size.y, 1.0);
    let uv = vec2<f32>(p.uv.x * aspect, p.uv.y);

    var col = vec3<f32>(0.035, 0.025, 0.07); // noite
    // Vermelho higanbana, laranja, violeta e azul-petróleo, cada um numa órbita própria.
    col += vec3<f32>(0.95, 0.12, 0.25) * aurora_blob(uv, vec2<f32>(aspect * (0.25 + 0.12 * sin(t * 1.3)), 0.35 + 0.18 * cos(t)), 0.38);
    col += vec3<f32>(1.00, 0.55, 0.10) * aurora_blob(uv, vec2<f32>(aspect * (0.70 + 0.15 * cos(t * 0.9)), 0.75 + 0.12 * sin(t * 1.7)), 0.30);
    col += vec3<f32>(0.55, 0.20, 0.95) * aurora_blob(uv, vec2<f32>(aspect * (0.55 + 0.20 * sin(t * 0.7 + 2.0)), 0.20 + 0.10 * sin(t * 1.1)), 0.34);
    col += vec3<f32>(0.05, 0.75, 0.70) * aurora_blob(uv, vec2<f32>(aspect * (0.88 + 0.08 * sin(t * 1.5)), 0.30 + 0.20 * cos(t * 0.8 + 1.0)), 0.26);
    return vec4<f32>(saturate(col), 1.0);
}
