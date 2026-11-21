/* gl_readback.c — leitura de pixel do back buffer antes do swap (usada por testes). */
#include "gl_internal.h"
#include "error.h"

hg_result gl_surface_read_pixel(gl_device* d, gl_surface* s, uint32_t x, uint32_t y, uint32_t* out) {
    hg_result r = gl_context_bind(&d->ctx, s->dc);
    if (r != HG_OK) return r;
    if (y >= s->height) return hg__fail(HG_ERR_INVALID, "opengl: pixel fora da superfície");
    const gl_api* gl = &d->ctx.gl;
    uint8_t bgra[4] = { 0 };
    gl->PixelStorei(GL_PACK_ALIGNMENT, 1);
    /* GL tem origem embaixo-esquerda: inverte Y. BGRA em memória = B no byte baixo. */
    gl->ReadPixels((GLint)x, (GLint)(s->height - 1 - y), 1, 1, GL_BGRA, GL_UNSIGNED_BYTE, bgra);
    *out = (uint32_t)bgra[0] | (uint32_t)bgra[1] << 8 | (uint32_t)bgra[2] << 16 | (uint32_t)bgra[3] << 24;
    return HG_OK;
}
