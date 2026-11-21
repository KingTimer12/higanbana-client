/* gl_shaders.c — porte GLSL 330 de quad.hlsl/glyph.hlsl e compilação de programas.
 * Coordenadas em pixels com origem no topo-esquerdo; o Y é invertido no clip space. */
#include "gl_internal.h"

#define GLSL(...) "#version 330 core\n" #__VA_ARGS__

const char* const hg__gl_quad_vs = GLSL(
    layout(location = 0) in vec4 a_rect;
    layout(location = 1) in vec4 a_fill;
    layout(location = 2) in vec4 a_border;
    layout(location = 3) in vec4 a_radius;
    layout(location = 4) in float a_bwidth;
    layout(location = 5) in float a_blur;
    layout(location = 6) in vec4 a_shadow;
    uniform vec2 u_viewport;
    out vec2 v_local;
    flat out vec2 v_half;
    flat out vec4 v_fill;
    flat out vec4 v_border;
    flat out vec4 v_radius;
    flat out float v_bwidth;
    flat out float v_blur;
    flat out vec4 v_shadow;
    void main() {
        vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
        float expand = a_shadow.a > 0.0 ? a_blur * 2.0 : 0.0;
        vec2 pos = a_rect.xy - expand + corner * (a_rect.zw + 2.0 * expand);
        gl_Position = vec4(pos / u_viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
        v_half = a_rect.zw * 0.5;
        v_local = pos - (a_rect.xy + v_half);
        v_fill = a_fill;
        v_border = a_border;
        v_radius = a_radius;
        v_bwidth = a_bwidth;
        v_blur = a_blur;
        v_shadow = a_shadow;
    }
);

const char* const hg__gl_quad_fs = GLSL(
    in vec2 v_local;
    flat in vec2 v_half;
    flat in vec4 v_fill;
    flat in vec4 v_border;
    flat in vec4 v_radius;
    flat in float v_bwidth;
    flat in float v_blur;
    flat in vec4 v_shadow;
    out vec4 o_color;
    float sd_round_rect(vec2 p, vec2 b, vec4 r) {
        float rr = p.y < 0.0 ? (p.x < 0.0 ? r.x : r.y) : (p.x < 0.0 ? r.w : r.z);
        rr = min(rr, min(b.x, b.y));
        vec2 q = abs(p) - b + rr;
        return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rr;
    }
    vec4 premul(vec4 c) { return vec4(c.rgb * c.a, c.a); }
    void main() {
        float d = sd_round_rect(v_local, v_half, v_radius);
        float coverage = clamp(0.5 - d, 0.0, 1.0);
        vec4 color = premul(v_fill);
        if (v_bwidth > 0.0) color = mix(color, premul(v_border), clamp(d + v_bwidth + 0.5, 0.0, 1.0));
        color *= coverage;
        if (v_shadow.a > 0.0 && v_blur > 0.0) {
            float s = 1.0 - smoothstep(-v_blur, v_blur, d);
            color += premul(v_shadow) * s * (1.0 - color.a);
        }
        o_color = color;
    }
);

const char* const hg__gl_glyph_vs = GLSL(
    layout(location = 0) in vec4 a_rect;
    layout(location = 1) in vec4 a_uv;
    layout(location = 2) in vec4 a_color;
    uniform vec2 u_viewport;
    uniform vec2 u_atlas_size;
    out vec2 v_uv;
    flat out vec4 v_color;
    void main() {
        vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
        vec2 pos = a_rect.xy + corner * a_rect.zw;
        gl_Position = vec4(pos / u_viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
        v_uv = (a_uv.xy + corner * a_uv.zw) / u_atlas_size;
        v_color = a_color;
    }
);

const char* const hg__gl_glyph_fs = GLSL(
    in vec2 v_uv;
    flat in vec4 v_color;
    uniform sampler2D u_atlas;
    out vec4 o_color;
    void main() {
        float coverage = texture(u_atlas, v_uv).r;
        o_color = vec4(v_color.rgb * v_color.a, v_color.a) * coverage;
    }
);

static GLuint compile(const gl_api* gl, GLenum type, const char* src) {
    GLuint sh = gl->CreateShader(type);
    GLint ok = 0;
    gl->ShaderSource(sh, 1, &src, NULL);
    gl->CompileShader(sh);
    gl->GetShaderiv(sh, GL_COMPILE_STATUS, &ok);
    if (!ok) { gl->DeleteShader(sh); return 0; }
    return sh;
}

GLuint gl_program_build(const gl_api* gl, const char* vs_src, const char* fs_src) {
    GLuint vs = compile(gl, GL_VERTEX_SHADER, vs_src);
    GLuint fs = compile(gl, GL_FRAGMENT_SHADER, fs_src);
    GLuint prog = 0;
    if (vs && fs) {
        GLint ok = 0;
        prog = gl->CreateProgram();
        gl->AttachShader(prog, vs);
        gl->AttachShader(prog, fs);
        gl->LinkProgram(prog);
        gl->GetProgramiv(prog, GL_LINK_STATUS, &ok);
        if (!ok) { gl->DeleteProgram(prog); prog = 0; }
    }
    if (vs) gl->DeleteShader(vs);
    if (fs) gl->DeleteShader(fs);
    return prog;
}
