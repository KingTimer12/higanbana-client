
// ---- fim do código do usuário ----
// entry.wgsl — pontos de entrada montados depois do código do usuário (ver compose.rs).

struct HgVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) screen: vec2<f32>, // pixels na janela, origem no topo-esquerda em todo backend
}

// Quad de 4 vértices (triangle strip) cobrindo `rect`; mesma conta de NDC dos quads.
@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> HgVsOut {
    let corner = vec2<f32>(f32(vid & 1u), f32(vid >> 1u));
    let pos = hg_u.rect.xy + corner * hg_u.rect.zw;
    var o: HgVsOut;
    o.pos = vec4<f32>(pos / hg_u.viewport.xy * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.screen = pos;
    return o;
}

// Chama `shade`, recorta os cantos (AA pela SDF), aplica opacidade e pré-multiplica.
@fragment
fn ps_main(i: HgVsOut) -> @location(0) vec4<f32> {
    let size = hg_u.rect.zw;
    let px = i.screen - hg_u.rect.xy;
    var p: ShadeInput;
    p.uv = px / max(size, vec2<f32>(1.0));
    p.px = px;
    p.size = size;
    p.screen = i.screen;
    p.time = hg_u.time_scale_opacity_flags.x;
    p.scale = hg_u.time_scale_opacity_flags.y;
    p.radius = hg_u.radius;
    let c = shade(p);
    let coverage = saturate(0.5 - rounded_sdf(px, size, hg_u.radius));
    let a = saturate(c.a) * coverage * hg_u.time_scale_opacity_flags.z;
    return vec4<f32>(c.rgb * a, a);
}
