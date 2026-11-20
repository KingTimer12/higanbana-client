/* gl_glyphs.c — textura GL_R8 do atlas (sob demanda) e upload das instâncias
 * coletadas por glyph_frame. */
#include "gl_internal.h"
#include "draw_list.h"
#include "error.h"

static void create_atlas(const gl_api* gl, gl_glyphs* g) {
    gl->GenTextures(1, &g->atlas);
    gl->BindTexture(GL_TEXTURE_2D, g->atlas);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    gl->TexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    gl->TexImage2D(GL_TEXTURE_2D, 0, GL_R8, HG_GLYPH_ATLAS_SIZE, HG_GLYPH_ATLAS_SIZE, 0, GL_RED,
                   GL_UNSIGNED_BYTE, NULL);
}

static hg_result upload_r8(void* user, hg_rect at, const uint8_t* r8, uint32_t pitch) {
    gl_device* d = user;
    const gl_api* gl = &d->ctx.gl;
    gl->BindTexture(GL_TEXTURE_2D, d->glyphs.atlas);
    gl->PixelStorei(GL_UNPACK_ALIGNMENT, 1);
    gl->PixelStorei(GL_UNPACK_ROW_LENGTH, (GLint)pitch);
    gl->TexSubImage2D(GL_TEXTURE_2D, 0, (GLint)at.x, (GLint)at.y, (GLsizei)at.w, (GLsizei)at.h, GL_RED,
                      GL_UNSIGNED_BYTE, r8);
    gl->PixelStorei(GL_UNPACK_ROW_LENGTH, 0);
    return HG_OK;
}

hg_result gl_glyphs_prepare(gl_device* d, hg_context* ctx, const hg_draw_list* dl) {
    const gl_api* gl = &d->ctx.gl;
    gl_glyphs* g = &d->glyphs;
    g->frame.count = 0;
    if (!hg__draw_list_has_text(dl)) return HG_OK;
    if (!g->atlas) create_atlas(gl, g);
    hg_result r = hg__glyph_frame_collect(&g->frame, ctx, dl, upload_r8, d);
    if (r != HG_OK || g->frame.count == 0) return r;
    uint32_t bytes = g->frame.count * (uint32_t)sizeof(hg_glyph_instance);
    gl_buffer_orphan(gl, &g->instances, bytes);
    gl->BufferSubData(GL_ARRAY_BUFFER, 0, (GLsizeiptr)bytes, g->frame.instances);
    return HG_OK;
}

void gl_glyphs_release(const gl_api* gl, gl_glyphs* g) {
    gl_buffer_release(gl, &g->instances);
    if (g->atlas) gl->DeleteTextures(1, &g->atlas);
    g->atlas = 0;
    hg__glyph_frame_free(&g->frame);
}
