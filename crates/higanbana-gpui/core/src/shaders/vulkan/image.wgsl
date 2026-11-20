// image.wgsl — imagens RGBA pré-multiplicadas com cantos arredondados (SDF) e tint
// para o backend Vulkan (mesma matemática de image.hlsl). Instância espelha
// hg_image_draw. set 0: binding 0 = textura da imagem, binding 1 = sampler linear.

struct Globals {
    viewport: vec2<f32>,
    atlas_size: vec2<f32>,
};
var<immediate> g: Globals;

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;

struct ImageInstance {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) radius: vec4<f32>,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) half_sz: vec2<f32>,
    @location(3) @interpolate(flat) tint: vec4<f32>,
    @location(4) @interpolate(flat) radius: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, i: ImageInstance) -> VSOut {
    let corner = vec2<f32>(f32(vid & 1u), f32(vid >> 1u));
    let pos = i.rect.xy + corner * i.rect.zw;
    var o: VSOut;
    o.pos = vec4<f32>(pos / g.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.uv = i.uv.xy + corner * i.uv.zw;
    o.half_sz = i.rect.zw * 0.5;
    o.local = pos - (i.rect.xy + o.half_sz);
    o.tint = i.tint;
    o.radius = i.radius;
    return o;
}

fn sd_round_rect(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var rr = select(select(r.z, r.w, p.x < 0.0), select(r.y, r.x, p.x < 0.0), p.y < 0.0);
    rr = min(rr, min(b.x, b.y));
    let q = abs(p) - b + rr;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}

@fragment
fn fs_main(i: VSOut) -> @location(0) vec4<f32> {
    let coverage = clamp(0.5 - sd_round_rect(i.local, i.half_sz, i.radius), 0.0, 1.0);
    let tint = vec4<f32>(i.tint.rgb * i.tint.a, i.tint.a);
    return textureSample(image, linear_sampler, i.uv) * tint * coverage;
}
