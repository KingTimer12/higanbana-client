// quad.wgsl — porte de quad.hlsl: retângulo arredondado por SDF com borda e sombra.
// Layout da instância espelha hg_quad (hgui.h).

struct Globals {
    viewport: vec2<f32>, // pixels físicos
    atlas_size: vec2<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

struct QuadInstance {
    @location(0) rect: vec4<f32>,    // x, y, w, h
    @location(1) fill: vec4<f32>,
    @location(2) border: vec4<f32>,
    @location(3) radius: vec4<f32>,  // tl, tr, br, bl
    @location(4) bw_blur: vec2<f32>, // border_width, shadow_blur
    @location(5) shadow: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) half_sz: vec2<f32>,
    @location(2) @interpolate(flat) fill: vec4<f32>,
    @location(3) @interpolate(flat) border: vec4<f32>,
    @location(4) @interpolate(flat) radius: vec4<f32>,
    @location(5) @interpolate(flat) bw_blur: vec2<f32>,
    @location(6) @interpolate(flat) shadow: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, q: QuadInstance) -> VsOut {
    let corner = vec2<f32>(f32(vid & 1u), f32(vid >> 1u));
    let expand = select(0.0, q.bw_blur.y * 2.0, q.shadow.a > 0.0);
    let pos = q.rect.xy - expand + corner * (q.rect.zw + 2.0 * expand);

    var o: VsOut;
    o.pos = vec4<f32>(pos / g.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.half_sz = q.rect.zw * 0.5;
    o.local = pos - (q.rect.xy + o.half_sz);
    o.fill = q.fill;
    o.border = q.border;
    o.radius = q.radius;
    o.bw_blur = q.bw_blur;
    o.shadow = q.shadow;
    return o;
}

fn sd_round_rect(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var rr = select(select(r.z, r.w, p.x < 0.0), select(r.y, r.x, p.x < 0.0), p.y < 0.0);
    rr = min(rr, min(b.x, b.y));
    let q = abs(p) - b + rr;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}

fn premul(c: vec4<f32>) -> vec4<f32> { return vec4<f32>(c.rgb * c.a, c.a); }

@fragment
fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    let d = sd_round_rect(i.local, i.half_sz, i.radius);
    let coverage = saturate(0.5 - d);

    var color = premul(i.fill);
    if (i.bw_blur.x > 0.0) {
        color = mix(color, premul(i.border), saturate(d + i.bw_blur.x + 0.5));
    }
    color = color * coverage;

    if (i.shadow.a > 0.0 && i.bw_blur.y > 0.0) {
        let s = 1.0 - smoothstep(-i.bw_blur.y, i.bw_blur.y, d);
        color = color + premul(i.shadow) * s * (1.0 - color.a);
    }
    return color;
}
