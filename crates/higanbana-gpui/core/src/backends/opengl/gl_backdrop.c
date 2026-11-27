/* gl_backdrop.c — textura por superfície com o que já foi desenhado atrás de um shader.
 * Só existe depois do 1º shader com HG_SHADER_BACKDROP; cada desenho copia apenas o
 * retângulo dele + margem, então dois vidros sobrepostos veem um ao outro.
 * A textura guarda as linhas como o FBO (origem embaixo): o prelúdio inverte o v
 * porque o uniform leva viewport.w = -1 (gl_user_shader.c). */
#include "gl_internal.h"
#include <math.h>

/* Unidade da textura de backdrop (binding 1 do mapa fixo). */
#define BACKDROP_UNIT 1

static int ensure_texture(const gl_api* gl, gl_surface* s) {
    gl_backdrop* b = &s->backdrop;
    if (b->tex && b->width == s->width && b->height == s->height) return 1;
    gl_backdrop_release(gl, b);
    gl->GenTextures(1, &b->tex);
    if (!b->tex) return 0;
    gl->BindTexture(GL_TEXTURE_2D, b->tex);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    gl->TexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, (GLsizei)s->width, (GLsizei)s->height, 0, GL_RGBA, GL_UNSIGNED_BYTE,
                   NULL);
    b->width = s->width;
    b->height = s->height;
    return 1;
}

static GLint clamp_px(float v, uint32_t max) {
    if (!(v > 0.0f)) return 0; /* também pega NaN */
    if (v > (float)max) return (GLint)max;
    return (GLint)v;
}

GLuint gl_backdrop_capture(gl_device* d, gl_surface* s, const hg_rect* r) {
    const float m = GL_BACKDROP_MARGIN;
    GLint left = clamp_px(floorf(r->x - m), s->width);
    GLint top = clamp_px(floorf(r->y - m), s->height);
    GLint right = clamp_px(ceilf(r->x + r->w + m), s->width);
    GLint bottom = clamp_px(ceilf(r->y + r->h + m), s->height);
    if (left >= right || top >= bottom) return 0; /* fora da superfície: nada a copiar */
    const gl_api* gl = &d->ctx.gl;
    gl->ActiveTexture(GL_TEXTURE0 + BACKDROP_UNIT); /* não mexe no atlas da unidade 0 */
    if (!ensure_texture(gl, s)) return 0;
    gl->BindTexture(GL_TEXTURE_2D, s->backdrop.tex);
    /* Topo-esquerda -> embaixo-esquerda; mesma posição no FBO e na textura. O scissor não afeta a cópia. */
    GLint y = (GLint)s->height - bottom;
    gl->CopyTexSubImage2D(GL_TEXTURE_2D, 0, left, y, left, y, right - left, bottom - top);
    return s->backdrop.tex;
}

uint64_t gl_backdrop_bytes(const gl_backdrop* b) {
    return b->tex ? (uint64_t)b->width * b->height * 4 : 0; /* RGBA8 */
}

void gl_backdrop_release(const gl_api* gl, gl_backdrop* b) {
    if (b->tex) gl->DeleteTextures(1, &b->tex);
    b->tex = 0;
    b->width = b->height = 0;
}
