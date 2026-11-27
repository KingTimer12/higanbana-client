/* opengl_backend.c — vtable do backend OpenGL 3.3 (WGL): liga contexto/janela aos módulos. */
#include "gl_internal.h"
#include "error.h"
#include <stdlib.h>

static gl_device* device_of(hg_context* ctx) { return ctx->backend_data; }
static gl_surface* surface_of(hg_window* win) { return win->surface; }

/* Cria e destrói um contexto 3.3 core, devolvendo o contexto que estava corrente. */
static int opengl_probe(hg_backend_info* info) {
    gl_wgl prev;
    if (!gl_wgl_open(&prev)) return 0;
    HGLRC prev_rc = prev.get_current_context();
    HDC prev_dc = prev.get_current_dc();
    gl_wgl_context c;
    int ok = gl_context_create(&c, info) == HG_OK;
    if (ok) gl_context_destroy(&c);
    if (prev_rc) prev.make_current(prev_dc, prev_rc);
    gl_wgl_close(&prev);
    return ok;
}

/* Sem contexto GL (o ICD não descarrega depois): opengl32 sempre existe, então
 * basta haver GPU de hardware, que traz o ICD do fabricante. */
static int opengl_detect(hg_backend_info* info) {
    return hg__detect_adapter(info);
}

static hg_result opengl_create(hg_context* ctx) {
    gl_device* d = calloc(1, sizeof *d);
    if (!d) return hg__fail(HG_ERR_OOM, "opengl: sem memória");
    hg_result r = gl_device_create(d, &ctx->info);
    if (r != HG_OK) { free(d); return r; }
    ctx->backend_data = d;
    return HG_OK;
}

static void opengl_destroy(hg_context* ctx) {
    gl_device* d = device_of(ctx);
    if (!d) return;
    gl_device_release(d);
    free(d);
    ctx->backend_data = NULL;
}

static hg_result opengl_attach(hg_context* ctx, hg_window* win) {
    gl_surface* s = calloc(1, sizeof *s);
    if (!s) return hg__fail(HG_ERR_OOM, "opengl: sem memória");
    hg_result r = gl_surface_create(device_of(ctx), win->hwnd, win->width, win->height, s);
    if (r != HG_OK) { gl_surface_release(device_of(ctx), s); free(s); return r; }
    win->surface = s;
    return HG_OK;
}

static void opengl_detach(hg_context* ctx, hg_window* win) {
    gl_surface* s = surface_of(win);
    if (!s) return;
    gl_surface_release(device_of(ctx), s);
    free(s);
    win->surface = NULL;
}

/* Viewport é aplicado no próximo begin_frame (0x0 ignorado lá). */
static hg_result opengl_resize(hg_window* win, uint32_t w, uint32_t h) {
    (void)win; (void)w; (void)h;
    return HG_OK;
}

static hg_result opengl_begin_frame(hg_window* win, hg_color clear) {
    return gl_surface_begin(device_of(win->ctx), surface_of(win), win->width, win->height, clear);
}

static hg_result opengl_submit(hg_window* win, const hg_draw_list* dl) {
    return gl_render_draw_list(device_of(win->ctx), win->ctx, surface_of(win), dl);
}

static hg_result opengl_end_frame(hg_window* win, int vsync) {
    return gl_surface_present(device_of(win->ctx), surface_of(win), vsync);
}

static hg_result opengl_read_pixel(hg_window* win, uint32_t x, uint32_t y, uint32_t* out) {
    return gl_surface_read_pixel(device_of(win->ctx), surface_of(win), x, y, out);
}

static hg_result opengl_tex_create(hg_context* ctx, struct hg_image* img) {
    return gl_texture_create(device_of(ctx), img);
}

static hg_result opengl_tex_update(hg_context* ctx, struct hg_image* img) {
    return gl_texture_update(device_of(ctx), img);
}

static void opengl_tex_destroy(hg_context* ctx, struct hg_image* img) {
    gl_texture_destroy(device_of(ctx), img);
}

static hg_result opengl_shader_prepare(hg_context* ctx, struct hg_shader* sh) {
    return gl_user_shader_prepare(device_of(ctx), sh);
}

static void opengl_shader_release(hg_context* ctx, struct hg_shader* sh) {
    gl_user_shader_release(device_of(ctx), sh);
}

const hg_backend_vtbl hg__opengl_backend = {
    .kind = HG_BACKEND_OPENGL,
    .name = "OpenGL",
    .probe = opengl_probe,
    .detect = opengl_detect,
    .create = opengl_create,
    .destroy = opengl_destroy,
    .attach = opengl_attach,
    .detach = opengl_detach,
    .resize = opengl_resize,
    .begin_frame = opengl_begin_frame,
    .submit = opengl_submit,
    .end_frame = opengl_end_frame,
    .texture_create = opengl_tex_create,
    .texture_update = opengl_tex_update,
    .texture_destroy = opengl_tex_destroy,
    .read_pixel = opengl_read_pixel,
    .shader_prepare = opengl_shader_prepare,
    .shader_release = opengl_shader_release,
};
