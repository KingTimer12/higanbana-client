/* gl_device.c — device = contexto WGL + pipeline + buffers; libera tudo com o contexto corrente. */
#include "gl_internal.h"
#include <string.h>

hg_result gl_device_create(gl_device* d, hg_backend_info* info) {
    memset(d, 0, sizeof *d);
    hg_result r = gl_context_create(&d->ctx, info);
    if (r != HG_OK) return r;
    r = gl_pipeline_create(&d->ctx.gl, &d->pipeline);
    if (r != HG_OK) gl_context_destroy(&d->ctx);
    return r;
}

void gl_device_release(gl_device* d) {
    if (!d->ctx.rc) return;
    if (gl_context_bind(&d->ctx, d->ctx.hidden_dc) == HG_OK) {
        const gl_api* gl = &d->ctx.gl;
        gl_glyphs_release(gl, &d->glyphs);
        gl_buffer_release(gl, &d->quads);
        gl_pipeline_release(gl, &d->pipeline);
    } else {
        hg__glyph_frame_free(&d->glyphs.frame); /* objetos GL morrem com o contexto */
    }
    gl_context_destroy(&d->ctx);
}
