// glyph.wgsl — glyphs do atlas R8 para o backend Vulkan (mesma matemática de glyph.hlsl).
// Instância espelha hg_glyph_instance. set 0: binding 0 = atlas, binding 1 = sampler nearest.

struct Globals {
    viewport: vec2<f32>,
    atlas_size: vec2<f32>,
};
var<immediate> g: Globals;

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var point_sampler: sampler;

struct GlyphInstance {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, gl: GlyphInstance) -> VSOut {
    let corner = vec2<f32>(f32(vid & 1u), f32(vid >> 1u));
    let pos = gl.rect.xy + corner * gl.rect.zw;
    var o: VSOut;
    o.pos = vec4<f32>(pos / g.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.uv = (gl.uv.xy + corner * gl.uv.zw) / g.atlas_size;
    o.color = gl.color;
    return o;
}

@fragment
fn fs_main(i: VSOut) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas, point_sampler, i.uv).r;
    return vec4<f32>(i.color.rgb * i.color.a, i.color.a) * coverage;
}
