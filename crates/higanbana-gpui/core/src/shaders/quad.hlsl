// quad.hlsl — retângulo arredondado por SDF com borda e sombra, instanciado.
// Layout da instância espelha hg_quad (hgui.h).

cbuffer Globals : register(b0) {
    float2 viewport; // pixels físicos
    float2 _pad;
};

struct QuadInstance {
    float4 rect    : RECT;    // x, y, w, h
    float4 fill    : FILL;
    float4 border  : BORDER;
    float4 radius  : RADIUS;  // tl, tr, br, bl
    float  bwidth  : BWIDTH;
    float  blur    : BLUR;
    float4 shadow  : SHADOW;
};

struct VSOut {
    float4 pos                     : SV_Position;
    float2 local                   : LOCAL;   // relativo ao centro
    nointerpolation float2 half_sz : HALFSZ;
    nointerpolation float4 fill    : FILL;
    nointerpolation float4 border  : BORDER;
    nointerpolation float4 radius  : RADIUS;
    nointerpolation float  bwidth  : BWIDTH;
    nointerpolation float  blur    : BLUR;
    nointerpolation float4 shadow  : SHADOW;
};

VSOut vs_main(uint vid : SV_VertexID, QuadInstance q) {
    float2 corner = float2(vid & 1, vid >> 1);
    float expand = q.shadow.a > 0 ? q.blur * 2.0 : 0.0;
    float2 pos = q.rect.xy - expand + corner * (q.rect.zw + 2.0 * expand);

    VSOut o;
    o.pos = float4(pos / viewport * float2(2, -2) + float2(-1, 1), 0, 1);
    o.half_sz = q.rect.zw * 0.5;
    o.local = pos - (q.rect.xy + o.half_sz);
    o.fill = q.fill;
    o.border = q.border;
    o.radius = q.radius;
    o.bwidth = q.bwidth;
    o.blur = q.blur;
    o.shadow = q.shadow;
    return o;
}

float sd_round_rect(float2 p, float2 b, float4 r) {
    float rr = p.y < 0 ? (p.x < 0 ? r.x : r.y) : (p.x < 0 ? r.w : r.z);
    rr = min(rr, min(b.x, b.y));
    float2 q = abs(p) - b + rr;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rr;
}

float4 premul(float4 c) { return float4(c.rgb * c.a, c.a); }

float4 ps_main(VSOut i) : SV_Target {
    float d = sd_round_rect(i.local, i.half_sz, i.radius);
    float coverage = saturate(0.5 - d);

    float4 color = premul(i.fill);
    if (i.bwidth > 0) color = lerp(color, premul(i.border), saturate(d + i.bwidth + 0.5));
    color *= coverage;

    if (i.shadow.a > 0 && i.blur > 0) {
        float s = 1.0 - smoothstep(-i.blur, i.blur, d);
        float4 sh = premul(i.shadow) * s;
        color += sh * (1.0 - color.a);
    }
    return color;
}
