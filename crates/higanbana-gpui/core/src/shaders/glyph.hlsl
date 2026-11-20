// glyph.hlsl — glyphs do atlas R8, instanciados. Layout espelha hg_glyph_instance.

cbuffer Globals : register(b0) {
    float2 viewport;
    float2 atlas_size;
};

Texture2D<float> atlas : register(t0);
SamplerState     point_sampler : register(s0);

struct GlyphInstance {
    float4 rect  : RECT;
    float4 uv    : UV;
    float4 color : COLOR;
};

struct VSOut {
    float4 pos                    : SV_Position;
    float2 uv                     : TEXCOORD0;
    nointerpolation float4 color  : COLOR;
};

VSOut vs_main(uint vid : SV_VertexID, GlyphInstance g) {
    float2 corner = float2(vid & 1, vid >> 1);
    float2 pos = g.rect.xy + corner * g.rect.zw;
    VSOut o;
    o.pos = float4(pos / viewport * float2(2, -2) + float2(-1, 1), 0, 1);
    o.uv = (g.uv.xy + corner * g.uv.zw) / atlas_size;
    o.color = g.color;
    return o;
}

float4 ps_main(VSOut i) : SV_Target {
    float coverage = atlas.Sample(point_sampler, i.uv);
    return float4(i.color.rgb * i.color.a, i.color.a) * coverage;
}
