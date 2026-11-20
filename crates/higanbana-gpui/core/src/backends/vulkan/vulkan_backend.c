/* vulkan_backend.c — vtable do backend Vulkan: liga contexto/janela aos módulos. */
#include "vk_internal.h"
#include "error.h"
#include <stdlib.h>

static vk_device* device_of(hg_context* ctx) { return ctx->backend_data; }
static vk_surface* surface_of(hg_window* win) { return win->surface; }

/* Instância de teste com surface Win32 + GPU que apresenta; nada fica vivo. */
static int vulkan_probe(hg_backend_info* info) {
    vk_api api;
    if (!vk_api_open(&api)) return 0;
    VkInstance inst = NULL;
    VkPhysicalDevice phys;
    uint32_t family;
    int ok = vk_instance_create(&api, &inst) == HG_OK &&
             vk_pick_physical(&api, inst, &phys, &family, info ? info->adapter : NULL, info ? sizeof info->adapter : 0);
    if (inst) api.vkDestroyInstance(inst, NULL);
    vk_api_close(&api);
    return ok;
}

static hg_result vulkan_create(hg_context* ctx) {
    vk_device* d = calloc(1, sizeof *d);
    if (!d) return hg__fail(HG_ERR_OOM, "vulkan: sem memória");
    hg_result r = vk_device_create(d, &ctx->info);
    if (r != HG_OK) { free(d); return r; }
    ctx->backend_data = d;
    return HG_OK;
}

static void vulkan_destroy(hg_context* ctx) {
    vk_device* d = device_of(ctx);
    if (!d) return;
    vk_device_release(d);
    free(d);
    ctx->backend_data = NULL;
}

static hg_result vulkan_attach(hg_context* ctx, hg_window* win) {
    vk_surface* s = calloc(1, sizeof *s);
    if (!s) return hg__fail(HG_ERR_OOM, "vulkan: sem memória");
    hg_result r = vk_surface_create(device_of(ctx), win->hwnd, win->width, win->height, s);
    if (r != HG_OK) { vk_surface_release(device_of(ctx), s); free(s); return r; }
    win->surface = s;
    return HG_OK;
}

static void vulkan_detach(hg_context* ctx, hg_window* win) {
    vk_surface* s = surface_of(win);
    if (!s) return;
    vk_surface_release(device_of(ctx), s);
    free(s);
    win->surface = NULL;
}

/* Recriação é preguiçosa: o próximo begin_frame compara o tamanho pedido. */
static hg_result vulkan_resize(hg_window* win, uint32_t w, uint32_t h) {
    (void)w; (void)h;
    surface_of(win)->dirty = 1;
    return HG_OK;
}

static hg_result vulkan_begin_frame(hg_window* win, hg_color clear) {
    return vk_frame_begin(device_of(win->ctx), surface_of(win), win->width, win->height, clear);
}

static hg_result vulkan_submit(hg_window* win, const hg_draw_list* dl) {
    return vk_render_draw_list(device_of(win->ctx), win->ctx, surface_of(win), dl);
}

static hg_result vulkan_end_frame(hg_window* win, int vsync) {
    return vk_frame_end(device_of(win->ctx), surface_of(win), vsync);
}

static hg_result vulkan_read_pixel(hg_window* win, uint32_t x, uint32_t y, uint32_t* out) {
    return vk_surface_read_pixel(device_of(win->ctx), surface_of(win), x, y, out);
}

const hg_backend_vtbl hg__vulkan_backend = {
    .kind = HG_BACKEND_VULKAN,
    .name = "Vulkan",
    .probe = vulkan_probe,
    .create = vulkan_create,
    .destroy = vulkan_destroy,
    .attach = vulkan_attach,
    .detach = vulkan_detach,
    .resize = vulkan_resize,
    .begin_frame = vulkan_begin_frame,
    .submit = vulkan_submit,
    .end_frame = vulkan_end_frame,
    .read_pixel = vulkan_read_pixel,
};
