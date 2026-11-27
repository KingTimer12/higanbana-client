/* gl_surface.c — superfície por janela: DC com o pixel format do device
 * (um GLRC serve todas as janelas), viewport, clear e SwapBuffers. */
#include "gl_internal.h"
#include "error.h"
#include <string.h>

hg_result gl_surface_create(gl_device* d, HWND hwnd, uint32_t w, uint32_t h, gl_surface* s) {
    memset(s, 0, sizeof *s);
    s->hwnd = hwnd;
    s->interval = -1;
    s->width = w;
    s->height = h;
    s->dc = GetDC(hwnd);
    if (!s->dc) return hg__fail(HG_ERR_DEVICE, "opengl: GetDC da janela falhou");
    /* Pixel format é definitivo por janela: reaproveita se já for o nosso (troca de backend). */
    int current = GetPixelFormat(s->dc);
    if (current == 0 && !SetPixelFormat(s->dc, d->ctx.pixel_format, &d->ctx.pfd))
        return hg__fail(HG_ERR_DEVICE, "opengl: SetPixelFormat da janela falhou");
    if (current != 0 && current != d->ctx.pixel_format)
        return hg__fail(HG_ERR_DEVICE, "opengl: janela já tem outro pixel format");
    return gl_context_bind(&d->ctx, s->dc);
}

void gl_surface_release(gl_device* d, gl_surface* s) {
    /* Não deixa o GLRC preso a uma DC que vai ser solta. */
    if (d && d->ctx.rc && d->ctx.wgl.get_current_dc() == s->dc) gl_context_bind(&d->ctx, d->ctx.hidden_dc);
    /* Backdrop é da surface: detach/troca de backend (HWND recriada) o soltam junto. */
    if (s->backdrop.tex && d && d->ctx.rc && gl_context_ensure_current(&d->ctx) == HG_OK)
        gl_backdrop_release(&d->ctx.gl, &s->backdrop);
    s->backdrop.tex = 0;
    if (s->dc) ReleaseDC(s->hwnd, s->dc);
    s->dc = NULL;
}

hg_result gl_surface_begin(gl_device* d, gl_surface* s, uint32_t w, uint32_t h, hg_color clear) {
    hg_result r = gl_context_bind(&d->ctx, s->dc);
    if (r != HG_OK) return r;
    const gl_api* gl = &d->ctx.gl;
    if (w && h && (w != s->width || h != s->height)) { /* 0x0 é ignorado */
        s->width = w;
        s->height = h;
        gl_backdrop_release(gl, &s->backdrop); /* tamanho velho: recriado no próximo uso */
    }
    gl->Viewport(0, 0, (GLsizei)s->width, (GLsizei)s->height);
    gl->Disable(GL_SCISSOR_TEST);
    gl->ClearColor(clear.r, clear.g, clear.b, clear.a);
    gl->Clear(GL_COLOR_BUFFER_BIT);
    return HG_OK;
}

hg_result gl_surface_present(gl_device* d, gl_surface* s, int vsync) {
    hg_result r = gl_context_bind(&d->ctx, s->dc);
    if (r != HG_OK) return r;
    int interval = vsync ? 1 : 0;
    if (d->ctx.wgl.swap_interval && s->interval != interval) {
        d->ctx.wgl.swap_interval(interval);
        s->interval = interval;
    }
    if (!SwapBuffers(s->dc)) return hg__fail(HG_ERR_DEVICE, "opengl: SwapBuffers falhou");
    return HG_OK;
}
