/* d3d12_backend.c — vtable do backend D3D12: liga contexto/janela aos módulos. */
#include "d3d12_internal.h"
#include "error.h"
#include <stdlib.h>

static d3d12_device* device_of(hg_context* ctx) { return ctx->backend_data; }
static d3d12_surface* surface_of(hg_window* win) { return win->surface; }

static void describe_first_adapter(const d3d12_loader* l, hg_backend_info* info) {
    IDXGIFactory4* f = NULL;
    IDXGIAdapter1* a = NULL;
    if (info && SUCCEEDED(l->create_factory2(0, &IID_IDXGIFactory4, (void**)&f)) &&
        SUCCEEDED(IDXGIFactory4_EnumAdapters1(f, 0, &a)))
        d3d12_describe_adapter(a, info);
    D3D12_RELEASE(a);
    D3D12_RELEASE(f);
}

static int d3d12_probe(hg_backend_info* info) {
    d3d12_loader l = { 0 };
    if (!d3d12_loader_open(&l)) return 0;
    int ok = SUCCEEDED(l.create_device(NULL, D3D_FEATURE_LEVEL_11_0, &IID_ID3D12Device, NULL));
    if (ok) describe_first_adapter(&l, info);
    d3d12_loader_close(&l);
    return ok;
}

/* Sem criar device (que carrega o driver): runtime presente + GPU de hardware. */
static int d3d12_detect(hg_backend_info* info) {
    return hg__detect_dll(L"d3d12.dll") && hg__detect_adapter(info);
}

static hg_result d3d12_create(hg_context* ctx) {
    d3d12_device* d = calloc(1, sizeof *d);
    if (!d) return hg__fail(HG_ERR_OOM, "d3d12: sem memória");
    hg_result r = d3d12_device_create(d, &ctx->info);
    if (r != HG_OK) { free(d); return r; }
    ctx->backend_data = d;
    return HG_OK;
}

static void d3d12_destroy(hg_context* ctx) {
    d3d12_device* d = device_of(ctx);
    if (!d) return;
    d3d12_device_release(d); /* espera a GPU antes de soltar */
    free(d);
    ctx->backend_data = NULL;
}

static hg_result d3d12_attach(hg_context* ctx, hg_window* win) {
    d3d12_surface* s = calloc(1, sizeof *s);
    if (!s) return hg__fail(HG_ERR_OOM, "d3d12: sem memória");
    hg_result r = d3d12_surface_create(device_of(ctx), win->hwnd, win->width, win->height, s);
    if (r != HG_OK) { d3d12_surface_release(device_of(ctx), s); free(s); return r; }
    win->surface = s;
    return HG_OK;
}

static void d3d12_detach(hg_context* ctx, hg_window* win) {
    d3d12_surface* s = surface_of(win);
    if (!s) return;
    d3d12_surface_release(device_of(ctx), s);
    free(s);
    win->surface = NULL;
}

static hg_result d3d12_resize(hg_window* win, uint32_t w, uint32_t h) {
    d3d12_device* d = device_of(win->ctx);
    if (d->recording) return HG_OK; /* aplicado no próximo begin_frame */
    return d3d12_surface_resize(d, surface_of(win), w, h);
}

static hg_result d3d12_begin_frame(hg_window* win, hg_color clear) {
    d3d12_device* d = device_of(win->ctx);
    d3d12_surface* s = surface_of(win);
    hg_result r = d3d12_surface_resize(d, s, win->width, win->height);
    if (r != HG_OK) return r;
    if (!s->buffers[0]) return hg__fail(HG_ERR_DEVICE, "d3d12: sem render target");
    return d3d12_frame_begin(d, s, clear);
}

static hg_result d3d12_submit(hg_window* win, const hg_draw_list* dl) {
    return d3d12_render_draw_list(device_of(win->ctx), win->ctx, surface_of(win), dl);
}

static hg_result d3d12_end_frame(hg_window* win, int vsync) {
    int occluded = 0;
    hg_result r = d3d12_frame_end(device_of(win->ctx), surface_of(win), vsync, &occluded);
    win->occluded = occluded; /* hg_window_occluded: o Rust não conta com o vsync */
    return r;
}

static hg_result d3d12_read_pixel(hg_window* win, uint32_t x, uint32_t y, uint32_t* out) {
    return d3d12_surface_read_pixel(device_of(win->ctx), surface_of(win), x, y, out);
}

static hg_result d3d12_tex_create(hg_context* ctx, struct hg_image* img) {
    return d3d12_texture_create(device_of(ctx), img);
}

static void d3d12_tex_destroy(hg_context* ctx, struct hg_image* img) {
    d3d12_texture_destroy(device_of(ctx), img);
}

static hg_result d3d12_shader_prepare(hg_context* ctx, struct hg_shader* sh) {
    return d3d12_user_shader_prepare(device_of(ctx), sh);
}

static void d3d12_shader_release(hg_context* ctx, struct hg_shader* sh) {
    d3d12_user_shader_release(device_of(ctx), sh);
}

const hg_backend_vtbl hg__d3d12_backend = {
    .kind = HG_BACKEND_D3D12,
    .name = "Direct3D 12",
    .probe = d3d12_probe,
    .detect = d3d12_detect,
    .create = d3d12_create,
    .destroy = d3d12_destroy,
    .attach = d3d12_attach,
    .detach = d3d12_detach,
    .resize = d3d12_resize,
    .begin_frame = d3d12_begin_frame,
    .submit = d3d12_submit,
    .end_frame = d3d12_end_frame,
    .texture_create = d3d12_tex_create,
    .texture_destroy = d3d12_tex_destroy,
    .read_pixel = d3d12_read_pixel,
    .shader_prepare = d3d12_shader_prepare,
    .shader_release = d3d12_shader_release,
};
