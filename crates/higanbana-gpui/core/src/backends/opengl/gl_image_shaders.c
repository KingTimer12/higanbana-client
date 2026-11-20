/* gl_image_shaders.c — porte GLSL 330 de image.hlsl: imagem RGBA pré-multiplicada,
 * cantos arredondados (SDF) e tint. Layout da instância espelha hg_image_draw. */
#include "gl_internal.h"

#define GLSL(...) "#version 330 core\n" #__VA_ARGS__

const char* const hg__gl_image_vs = GLSL(
    layout(location = 0) in vec4 a_rect;
    layout(location = 1) in vec4 a_uv;
    layout(location = 2) in vec4 a_tint;
    layout(location = 3) in vec4 a_radius;
    uniform vec2 u_viewport;
    out vec2 v_uv;
    out vec2 v_local;
    flat out vec2 v_half;
    flat out vec4 v_tint;
    flat out vec4 v_radius;
    void main() {
        vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
        vec2 pos = a_rect.xy + corner * a_rect.zw;
        gl_Position = vec4(pos / u_viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
        v_uv = a_uv.xy + corner * a_uv.zw;
        v_half = a_rect.zw * 0.5;
        v_local = pos - (a_rect.xy + v_half);
        v_tint = a_tint;
        v_radius = a_radius;
    }
);

const char* const hg__gl_image_fs = GLSL(
    in vec2 v_uv;
    in vec2 v_local;
    flat in vec2 v_half;
    flat in vec4 v_tint;
    flat in vec4 v_radius;
    uniform sampler2D u_image;
    out vec4 o_color;
    float sd_round_rect(vec2 p, vec2 b, vec4 r) {
        float rr = p.y < 0.0 ? (p.x < 0.0 ? r.x : r.y) : (p.x < 0.0 ? r.w : r.z);
        rr = min(rr, min(b.x, b.y));
        vec2 q = abs(p) - b + rr;
        return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rr;
    }
    void main() {
        float coverage = clamp(0.5 - sd_round_rect(v_local, v_half, v_radius), 0.0, 1.0);
        vec4 tint = vec4(v_tint.rgb * v_tint.a, v_tint.a);
        o_color = texture(u_image, v_uv) * tint * coverage;
    }
);
