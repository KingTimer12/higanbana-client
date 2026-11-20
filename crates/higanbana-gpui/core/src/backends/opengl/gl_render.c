/* gl_render.c — sobe todos os quads num VBO órfão e desenha um lote por clip
 * (quads, depois texto), com scissor convertido para origem embaixo-esquerda. */
#include "gl_internal.h"
#include "draw_list.h"
#include "text.h"

static void upload_quads(gl_device* d, const hg_draw_list* dl) {
    const gl_api* gl = &d->ctx.gl;
    gl_buffer_orphan(gl, &d->quads, dl->quad_count * (uint32_t)sizeof(hg_quad));
    GLintptr at = 0;
    for (const hg_batch* b = dl->first; b; b = b->next)
        for (const hg_quad_chunk* c = b->quads_first; c; c = c->next) {
            GLsizeiptr bytes = (GLsizeiptr)(sizeof(hg_quad) * c->count);
            gl->BufferSubData(GL_ARRAY_BUFFER, at, bytes, c->items);
            at += bytes;
        }
}

static void set_scissor(const gl_api* gl, const hg_batch* b, const gl_surface* s) {
    GLint x = 0, y = 0;
    GLsizei w = (GLsizei)s->width, h = (GLsizei)s->height;
    if (b->has_clip) {
        x = (GLint)b->clip.x;
        w = b->clip.w > 0 ? (GLsizei)b->clip.w : 0;
        h = b->clip.h > 0 ? (GLsizei)b->clip.h : 0;
        y = (GLint)s->height - (GLint)(b->clip.y + b->clip.h);
    }
    gl->Scissor(x, y, w, h);
}

static void bind_common(gl_device* d, const gl_surface* s) {
    const gl_api* gl = &d->ctx.gl;
    const gl_pipeline* p = &d->pipeline;
    gl->Enable(GL_BLEND);
    gl->BlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA); /* alfa pré-multiplicado */
    gl->Enable(GL_SCISSOR_TEST);
    gl->UseProgram(p->quad_prog);
    gl->Uniform2f(p->quad_viewport, (GLfloat)s->width, (GLfloat)s->height);
    gl->UseProgram(p->glyph_prog);
    gl->Uniform2f(p->glyph_viewport, (GLfloat)s->width, (GLfloat)s->height);
    gl->Uniform2f(p->glyph_atlas_size, (GLfloat)HG_GLYPH_ATLAS_SIZE, (GLfloat)HG_GLYPH_ATLAS_SIZE);
    gl->ActiveTexture(GL_TEXTURE0);
    gl->BindTexture(GL_TEXTURE_2D, d->glyphs.atlas);
}

hg_result gl_render_draw_list(gl_device* d, hg_context* ctx, gl_surface* s, const hg_draw_list* dl) {
    hg_result r = gl_context_bind(&d->ctx, s->dc);
    if (r == HG_OK) r = gl_glyphs_prepare(d, ctx, dl);
    if (r != HG_OK) return r;
    if (dl->quad_count > 0) upload_quads(d, dl);
    const gl_api* gl = &d->ctx.gl;
    const gl_pipeline* p = &d->pipeline;
    bind_common(d, s);
    ctx->stats.draw_calls = 0;

    uint32_t quad_start = 0, glyph_start = 0, i = 0;
    for (const hg_batch* b = dl->first; b; b = b->next, i++) {
        uint32_t glyphs = d->glyphs.frame.count ? d->glyphs.frame.batch_counts[i] : 0;
        if (b->quad_count == 0 && glyphs == 0) continue;
        set_scissor(gl, b, s);
        if (b->quad_count) {
            gl->UseProgram(p->quad_prog);
            gl_pipeline_point_quads(gl, p, d->quads.name, quad_start);
            gl->DrawArraysInstanced(GL_TRIANGLE_STRIP, 0, 4, (GLsizei)b->quad_count);
            ctx->stats.draw_calls++;
            quad_start += b->quad_count;
        }
        if (glyphs) {
            gl->UseProgram(p->glyph_prog);
            gl_pipeline_point_glyphs(gl, p, d->glyphs.instances.name, glyph_start);
            gl->DrawArraysInstanced(GL_TRIANGLE_STRIP, 0, 4, (GLsizei)glyphs);
            ctx->stats.draw_calls++;
            glyph_start += glyphs;
        }
    }
    gl->BindVertexArray(0);
    ctx->stats.gpu_bytes = gl_gpu_bytes(d);
    return HG_OK;
}
