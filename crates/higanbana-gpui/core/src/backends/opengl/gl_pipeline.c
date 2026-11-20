/* gl_pipeline.c — programas de quad/glyph, VAOs e ponteiros de atributos por instância. */
#include "gl_internal.h"
#include "error.h"
#include "text.h"
#include <stddef.h>

typedef struct attrib { GLuint loc; GLint size; size_t offset; } attrib;

static const attrib k_quad_attribs[] = {
    { 0, 4, offsetof(hg_quad, rect) },   { 1, 4, offsetof(hg_quad, fill) },
    { 2, 4, offsetof(hg_quad, border) }, { 3, 4, offsetof(hg_quad, radius) },
    { 4, 1, offsetof(hg_quad, border_width) }, { 5, 1, offsetof(hg_quad, shadow_blur) },
    { 6, 4, offsetof(hg_quad, shadow) },
};

static const attrib k_glyph_attribs[] = {
    { 0, 4, offsetof(hg_glyph_instance, rect) },
    { 1, 4, offsetof(hg_glyph_instance, uv) },
    { 2, 4, offsetof(hg_glyph_instance, color) },
};

static GLuint make_vao(const gl_api* gl, const attrib* a, size_t n) {
    GLuint vao = 0;
    gl->GenVertexArrays(1, &vao);
    gl->BindVertexArray(vao);
    for (size_t i = 0; i < n; i++) {
        gl->EnableVertexAttribArray(a[i].loc);
        gl->VertexAttribDivisor(a[i].loc, 1);
    }
    gl->BindVertexArray(0);
    return vao;
}

/* Ponteiros relativos à instância `first`: substitui o base instance do GL 4.2. */
static void point(const gl_api* gl, GLuint vao, GLuint vbo, const attrib* a, size_t n, GLsizei stride, uint32_t first) {
    gl->BindVertexArray(vao);
    gl->BindBuffer(GL_ARRAY_BUFFER, vbo);
    size_t base = (size_t)first * (size_t)stride;
    for (size_t i = 0; i < n; i++)
        gl->VertexAttribPointer(a[i].loc, a[i].size, GL_FLOAT, GL_FALSE, stride, (const void*)(base + a[i].offset));
}

void gl_pipeline_point_quads(const gl_api* gl, const gl_pipeline* p, GLuint vbo, uint32_t first) {
    point(gl, p->quad_vao, vbo, k_quad_attribs, ARRAYSIZE(k_quad_attribs), sizeof(hg_quad), first);
}

void gl_pipeline_point_glyphs(const gl_api* gl, const gl_pipeline* p, GLuint vbo, uint32_t first) {
    point(gl, p->glyph_vao, vbo, k_glyph_attribs, ARRAYSIZE(k_glyph_attribs), sizeof(hg_glyph_instance), first);
}

hg_result gl_pipeline_create(const gl_api* gl, gl_pipeline* p) {
    p->quad_prog = gl_program_build(gl, hg__gl_quad_vs, hg__gl_quad_fs);
    p->glyph_prog = gl_program_build(gl, hg__gl_glyph_vs, hg__gl_glyph_fs);
    if (!p->quad_prog || !p->glyph_prog) {
        gl_pipeline_release(gl, p);
        return hg__fail(HG_ERR_DEVICE, "opengl: falha ao compilar shaders");
    }
    p->quad_viewport = gl->GetUniformLocation(p->quad_prog, "u_viewport");
    p->glyph_viewport = gl->GetUniformLocation(p->glyph_prog, "u_viewport");
    p->glyph_atlas_size = gl->GetUniformLocation(p->glyph_prog, "u_atlas_size");
    gl->UseProgram(p->glyph_prog);
    gl->Uniform1i(gl->GetUniformLocation(p->glyph_prog, "u_atlas"), 0);
    gl->UseProgram(0);
    p->quad_vao = make_vao(gl, k_quad_attribs, ARRAYSIZE(k_quad_attribs));
    p->glyph_vao = make_vao(gl, k_glyph_attribs, ARRAYSIZE(k_glyph_attribs));
    return HG_OK;
}

void gl_pipeline_release(const gl_api* gl, gl_pipeline* p) {
    if (p->quad_vao) gl->DeleteVertexArrays(1, &p->quad_vao);
    if (p->glyph_vao) gl->DeleteVertexArrays(1, &p->glyph_vao);
    if (p->quad_prog) gl->DeleteProgram(p->quad_prog);
    if (p->glyph_prog) gl->DeleteProgram(p->glyph_prog);
    p->quad_vao = p->glyph_vao = p->quad_prog = p->glyph_prog = 0;
}
