/* d3d11_backend.c — vtable do backend D3D11: liga contexto/janela aos módulos. */
#include "d3d11_internal.h"
#include "error.h"
#include <stdlib.h>

static d3d11_device* device_of(hg_context* ctx) { return ctx->backend_data; }
static d3d11_surface* surface_of(hg_window* win) { return win->surface; }

static int d3d11_probe(hg_backend_info* info) {
    (void)info;
    d3d11_loader l;
    if (!d3d11_loader_open(&l)) return 0;
    d3d11_loader_close(&l);
    return 1;
}

static hg_result d3d11_create(hg_context* ctx) {
    d3d11_device* d = calloc(1, sizeof *d);
    if (!d) return hg__fail(HG_ERR_OOM, "d3d11: sem memória");
    hg_result r = d3d11_device_create(d, &ctx->info);
    if (r != HG_OK) { free(d); return r; }
    ctx->backend_data = d;
    return HG_OK;
}

static void d3d11_destroy(hg_context* ctx) {
    d3d11_device* d = device_of(ctx);
    if (!d) return;
    d3d11_device_release(d);
    free(d);
    ctx->backend_data = NULL;
}

static hg_result d3d11_attach(hg_context* ctx, hg_window* win) {
    d3d11_surface* s = calloc(1, sizeof *s);
    if (!s) return hg__fail(HG_ERR_OOM, "d3d11: sem memória");
    hg_result r = d3d11_surface_create(device_of(ctx), win->hwnd, win->width, win->height, s);
    if (r != HG_OK) { d3d11_surface_release(s); free(s); return r; }
    win->surface = s;
    return HG_OK;
}

static void d3d11_detach(hg_context* ctx, hg_window* win) {
    (void)ctx;
    d3d11_surface* s = surface_of(win);
    if (!s) return;
    d3d11_surface_release(s);
    free(s);
    win->surface = NULL;
}

static hg_result d3d11_resize(hg_window* win, uint32_t w, uint32_t h) {
    return d3d11_surface_resize(device_of(win->ctx), surface_of(win), w, h);
}

static hg_result d3d11_begin_frame(hg_window* win, hg_color clear) {
    d3d11_device* d = device_of(win->ctx);
    d3d11_surface* s = surface_of(win);
    hg_result r = d3d11_surface_resize(d, s, win->width, win->height);
    if (r != HG_OK) return r;
    if (!s->rtv) return hg__fail(HG_ERR_DEVICE, "d3d11: sem render target");
    const float c[4] = { clear.r, clear.g, clear.b, clear.a };
    ID3D11DeviceContext_OMSetRenderTargets(d->imm, 1, &s->rtv, NULL);
    ID3D11DeviceContext_ClearRenderTargetView(d->imm, s->rtv, c);
    return HG_OK;
}

static hg_result d3d11_submit(hg_window* win, const hg_draw_list* dl) {
    return d3d11_render_draw_list(device_of(win->ctx), win->ctx, surface_of(win), dl);
}

static hg_result d3d11_end_frame(hg_window* win, int vsync) {
    return d3d11_surface_present(surface_of(win), vsync);
}

static hg_result d3d11_read_pixel(hg_window* win, uint32_t x, uint32_t y, uint32_t* out) {
    return d3d11_surface_read_pixel(device_of(win->ctx), surface_of(win), x, y, out);
}

static hg_result d3d11_tex_create(hg_context* ctx, struct hg_image* img) {
    return d3d11_texture_create(device_of(ctx), img);
}

static hg_result d3d11_tex_update(hg_context* ctx, struct hg_image* img) {
    return d3d11_texture_update(device_of(ctx), img);
}

static void d3d11_tex_destroy(hg_context* ctx, struct hg_image* img) {
    d3d11_texture_destroy(device_of(ctx), img);
}

const hg_backend_vtbl hg__d3d11_backend = {
    .kind = HG_BACKEND_D3D11,
    .name = "Direct3D 11",
    .probe = d3d11_probe,
    .detect = d3d11_probe, /* já é barato: só abre a d3d11.dll */
    .create = d3d11_create,
    .destroy = d3d11_destroy,
    .attach = d3d11_attach,
    .detach = d3d11_detach,
    .resize = d3d11_resize,
    .begin_frame = d3d11_begin_frame,
    .submit = d3d11_submit,
    .end_frame = d3d11_end_frame,
    .texture_create = d3d11_tex_create,
    .texture_destroy = d3d11_tex_destroy,
    .read_pixel = d3d11_read_pixel,
    .texture_update = d3d11_tex_update,
};
