// image.hlsl — imagens RGBA pré-multiplicadas com cantos arredondados (SDF) e tint.
// Layout da instância espelha hg_image_draw (hgui.h).

cbuffer Globals : register(b0) {
    float2 viewport;
    float2 _pad;
};

Texture2D    image : register(t0);
SamplerState linear_sampler : register(s0);

struct ImageInstance {
    float4 rect   : RECT;
    float4 uv     : UV;     // x, y, w, h em 0..1
    float4 tint   : TINT;
    float4 radius : RADIUS; // tl, tr, br, bl
};

struct VSOut {
    float4 pos                     : SV_Position;
    float2 uv                      : TEXCOORD0;
    float2 local                   : LOCAL;
    nointerpolation float2 half_sz : HALFSZ;
    nointerpolation float4 tint    : TINT;
    nointerpolation float4 radius  : RADIUS;
};

VSOut vs_main(uint vid : SV_VertexID, ImageInstance i) {
    float2 corner = float2(vid & 1, vid >> 1);
    float2 pos = i.rect.xy + corner * i.rect.zw;
    VSOut o;
    o.pos = float4(pos / viewport * float2(2, -2) + float2(-1, 1), 0, 1);
    o.uv = i.uv.xy + corner * i.uv.zw;
    o.half_sz = i.rect.zw * 0.5;
    o.local = pos - (i.rect.xy + o.half_sz);
    o.tint = i.tint;
    o.radius = i.radius;
    return o;
}

float sd_round_rect(float2 p, float2 b, float4 r) {
    float rr = p.y < 0 ? (p.x < 0 ? r.x : r.y) : (p.x < 0 ? r.w : r.z);
    rr = min(rr, min(b.x, b.y));
    float2 q = abs(p) - b + rr;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rr;
}

float4 ps_main(VSOut i) : SV_Target {
    float coverage = saturate(0.5 - sd_round_rect(i.local, i.half_sz, i.radius));
    float4 tint = float4(i.tint.rgb * i.tint.a, i.tint.a);
    return image.Sample(linear_sampler, i.uv) * tint * coverage;
}
