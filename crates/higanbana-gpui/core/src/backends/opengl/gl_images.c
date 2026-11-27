/* gl_images.c — texturas GL_RGBA8 de imagem, programa de imagem (sob demanda)
 * e desenho agrupado por textura (vizinhas iguais = um draw instanciado). */
#include "gl_internal.h"
#include "draw_list.h"
#include "error.h"
#include "image.h"
#include <stddef.h>
#include <stdlib.h>

static const size_t k_offsets[] = {
    offsetof(hg_image_draw, rect), offsetof(hg_image_draw, uv),
    offsetof(hg_image_draw, tint), offsetof(hg_image_draw, radius),
};

/* create/destroy podem vir fora de um frame (troca de backend, destroy). */
static hg_result ensure_current(gl_device* d) {
    return gl_context_ensure_current(&d->ctx);
}

hg_result gl_texture_create(gl_device* d, hg_image* img) {
    hg_result r = ensure_current(d);
    if (r != HG_OK) return r;
    const gl_api* gl = &d->ctx.gl;
    GLuint* t = calloc(1, sizeof *t);
    if (!t) return hg__fail(HG_ERR_OOM, "opengl: sem memória para textura");
    gl->GenTextures(1, t);
    if (!*t) { free(t); return hg__fail(HG_ERR_DEVICE, "opengl: glGenTextures falhou"); }
    gl->BindTexture(GL_TEXTURE_2D, *t);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    gl->PixelStorei(GL_UNPACK_ALIGNMENT, 4);
    gl->PixelStorei(GL_UNPACK_ROW_LENGTH, 0);
    gl->TexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, (GLsizei)img->width, (GLsizei)img->height, 0, GL_RGBA,
                   GL_UNSIGNED_BYTE, img->pixels);
    img->gpu = t;
    d->images.texture_bytes += (uint64_t)img->width * img->height * 4;
    return HG_OK;
}

hg_result gl_texture_update(gl_device* d, hg_image* img) {
    GLuint* t = img->gpu;
    if (!t) return HG_OK;
    hg_result r = ensure_current(d);
    if (r != HG_OK) return r;
    const gl_api* gl = &d->ctx.gl;
    gl->BindTexture(GL_TEXTURE_2D, *t);
    gl->PixelStorei(GL_UNPACK_ALIGNMENT, 4);
    gl->PixelStorei(GL_UNPACK_ROW_LENGTH, 0);
    gl->TexSubImage2D(GL_TEXTURE_2D, 0, 0, 0, (GLsizei)img->width, (GLsizei)img->height, GL_RGBA, GL_UNSIGNED_BYTE, img->pixels);
    return HG_OK;
}

void gl_texture_destroy(gl_device* d, hg_image* img) {
    GLuint* t = img->gpu;
    if (!t) return;
    if (ensure_current(d) == HG_OK) d->ctx.gl.DeleteTextures(1, t);
    free(t);
    img->gpu = NULL;
    d->images.texture_bytes -= (uint64_t)img->width * img->height * 4;
}

static hg_result ensure_pipeline(const gl_api* gl, gl_images* p) {
    if (p->vao) return HG_OK;
    p->prog = gl_program_build(gl, hg__gl_image_vs, hg__gl_image_fs);
    if (!p->prog) return hg__fail(HG_ERR_DEVICE, "opengl: falha ao compilar shader de imagem");
    p->viewport = gl->GetUniformLocation(p->prog, "u_viewport");
    gl->UseProgram(p->prog);
    gl->Uniform1i(gl->GetUniformLocation(p->prog, "u_image"), 0);
    gl->GenVertexArrays(1, &p->vao);
    gl->BindVertexArray(p->vao);
    for (GLuint i = 0; i < ARRAYSIZE(k_offsets); i++) {
        gl->EnableVertexAttribArray(i);
        gl->VertexAttribDivisor(i, 1);
    }
    gl->BindVertexArray(0);
    return HG_OK;
}

void gl_images_release(const gl_api* gl, gl_images* p) {
    gl_buffer_release(gl, &p->instances);
    if (p->vao) gl->DeleteVertexArrays(1, &p->vao);
    if (p->prog) gl->DeleteProgram(p->prog);
    p->vao = p->prog = 0;
}

hg_result gl_images_prepare(gl_device* d, const hg_draw_list* dl) {
    if (dl->image_count == 0) return HG_OK;
    const gl_api* gl = &d->ctx.gl;
    hg_result r = ensure_pipeline(gl, &d->images);
    for (const hg_batch* b = dl->first; b && r == HG_OK; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c && r == HG_OK; c = c->next)
            for (uint32_t i = 0; i < c->count && r == HG_OK; i++) r = hg__image_gpu(c->images[i]);
    if (r != HG_OK) return r;
    gl_buffer_orphan(gl, &d->images.instances, dl->image_count * (uint32_t)sizeof(hg_image_draw));
    GLintptr at = 0;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_image_chunk* c = b->images_first; c; c = c->next) {
            GLsizeiptr bytes = (GLsizeiptr)(sizeof(hg_image_draw) * c->count);
            gl->BufferSubData(GL_ARRAY_BUFFER, at, bytes, c->items);
            at += bytes;
        }
    return HG_OK;
}

/* Sem base instance no 3.3: reaponta os atributos no byte da instância `first`. */
static void draw_run(const gl_api* gl, const hg_image* img, uint32_t first, uint32_t count) {
    size_t base = (size_t)first * sizeof(hg_image_draw);
    for (GLuint i = 0; i < ARRAYSIZE(k_offsets); i++)
        gl->VertexAttribPointer(i, 4, GL_FLOAT, GL_FALSE, sizeof(hg_image_draw), (const void*)(base + k_offsets[i]));
    gl->BindTexture(GL_TEXTURE_2D, *(const GLuint*)img->gpu);
    gl->DrawArraysInstanced(GL_TRIANGLE_STRIP, 0, 4, (GLsizei)count);
}

uint32_t gl_images_draw(gl_device* d, const gl_surface* s, const hg_batch* b, uint32_t start) {
    const gl_api* gl = &d->ctx.gl;
    gl->UseProgram(d->images.prog);
    gl->Uniform2f(d->images.viewport, (GLfloat)s->width, (GLfloat)s->height);
    gl->BindVertexArray(d->images.vao);
    gl->BindBuffer(GL_ARRAY_BUFFER, d->images.instances.name);
    const hg_image* run_img = NULL;
    uint32_t run_start = start, run_len = 0, calls = 0;
    for (const hg_image_chunk* ch = b->images_first; ch; ch = ch->next)
        for (uint32_t i = 0; i < ch->count; i++) {
            if (ch->images[i] == run_img) { run_len++; continue; }
            if (run_len) { draw_run(gl, run_img, run_start, run_len); calls++; }
            run_img = ch->images[i];
            run_start += run_len;
            run_len = 1;
        }
    if (run_len) { draw_run(gl, run_img, run_start, run_len); calls++; }
    gl->BindTexture(GL_TEXTURE_2D, d->glyphs.atlas); /* texto do lote usa a unidade 0 */
    return calls;
}
